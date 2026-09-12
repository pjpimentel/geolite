use rusqlite::Connection;

use super::entity::admin_level;
use super::rules::extraction_rules;
use super::scale::level;
use super::{place_ways, relations, streets};

pub struct extract_opts<'a> {
  pub threads: usize,
  pub name_priority: &'a [&'a str],
  pub rules: &'a [extraction_rules],
}

pub struct progress_report {
  pub total: Option<u64>,
  pub processed: u64,
}

pub(super) const CHUNK_SIZE: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum source {
  relations,
  place_ways,
  streets,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct stage {
  pub level: level,
  pub source: source,
  pub ordinal: usize,
  pub of: usize,
}

pub struct extract_event {
  pub stage: stage,
  pub step: extract_step,
}

pub enum extract_step {
  started,
  candidates { found: u64, pending: u64 },
  progress(progress_report),
  finished,
}

impl admin_level {
  pub fn stages_of(level: level) -> Vec<stage> {
    let sources: &[source] = match level {
      level::neighborhood => &[source::relations, source::place_ways],
      level::street => &[source::streets],
      _ => &[source::relations],
    };
    sources
      .iter()
      .enumerate()
      .map(|(i, &source)| stage {
        level,
        source,
        ordinal: i + 1,
        of: sources.len(),
      })
      .collect()
  }

  pub fn extract(
    conn: &Connection,
    level: level,
    opts: &extract_opts,
    mut on_event: impl FnMut(extract_event),
  ) {
    for stage in Self::stages_of(level) {
      let mut emit = |step: extract_step| on_event(extract_event { stage, step });
      emit(extract_step::started);
      match stage.source {
        source::relations => {
          let found =
            crate::database::osm_relations::all_ids_by_admin_level(conn, level.value()).len() as u64;
          let pending =
            crate::database::osm_relations::remaining_ids_by_admin_level(conn, level.value());
          emit(extract_step::candidates {
            found,
            pending: pending.len() as u64,
          });
          relations::run_with_ids(
            conn,
            pending,
            level,
            opts.threads,
            opts.name_priority,
            |report| emit(extract_step::progress(report)),
          );
        }
        source::place_ways => place_ways::run(conn, opts.rules, opts.name_priority, |report| {
          emit(extract_step::progress(report))
        }),
        source::streets => streets::run(conn, opts.rules, opts.name_priority, |report| {
          emit(extract_step::progress(report))
        }),
      }
      emit(extract_step::finished);
    }
  }
}

#[cfg(test)]
#[path = "extract.test.rs"]
mod tests;
