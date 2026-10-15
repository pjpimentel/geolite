use std::collections::HashSet;

use geo::Geometry;
use rusqlite::Connection;

use super::country_index::country_index;
use super::entity::admin_level;
use super::scale::level;
use super::{place_ways, relations, streets};
use crate::database::store::base_pool;
use crate::progress_report;

pub struct extract_opts<'a> {
  pub threads: usize,
  pub name_priority: Option<&'a [&'a str]>,
  pub name_tags: &'a [&'a str],
  pub countries: &'a country_index,
}

pub(super) struct identity {
  pub(super) name: String,
  pub(super) iso: &'static str,
}

pub(super) fn identity_of(
  opts: &extract_opts,
  level: level,
  geometry: &Geometry<f64>,
  names: &[Option<String>],
) -> Option<identity> {
  let iso = opts.countries.country_of(geometry)?;
  let preset = crate::presets::for_country(Some(iso)).extract_osm_admin_levels;
  if !preset.admin_levels.contains(&level) {
    return None;
  }
  let priority = opts.name_priority.unwrap_or(preset.name_priority);
  let name = name_by_priority(names, opts.name_tags, priority)?;
  Some(identity { name, iso })
}

fn name_by_priority(
  names: &[Option<String>],
  name_tags: &[&str],
  priority: &[&str],
) -> Option<String> {
  priority.iter().find_map(|tag| {
    let at = name_tags.iter().position(|known| known == tag)?;
    names
      .get(at)?
      .as_deref()
      .map(str::trim)
      .filter(|name| !name.is_empty())
      .map(str::to_owned)
  })
}

pub(super) struct written {
  pub(super) kept: u64,
  pub(super) dropped: u64,
}

// a batch goes to the base of each row's country; a sealed base takes none, and its rows are
// dropped and counted
pub(super) fn write_batch(pool: &mut base_pool, batch: Vec<admin_level>) -> written {
  let mut by_iso: Vec<(String, Vec<admin_level>)> = Vec::new();
  for row in batch {
    let iso = row.country_iso_code.clone().unwrap_or_default();
    match by_iso.iter_mut().find(|(known, _)| *known == iso) {
      Some((_, rows)) => rows.push(row),
      None => by_iso.push((iso, vec![row])),
    }
  }
  let mut written = written {
    kept: 0,
    dropped: 0,
  };
  for (iso, rows) in by_iso {
    match pool.conn_for(&iso) {
      Some(conn) => written.kept += super::repository::batch_upsert(conn, &rows) as u64,
      None => written.dropped += rows.len() as u64,
    }
  }
  written
}

// what the bases already hold at a level: a resume subtracts it from the candidates
pub(super) fn known_relation_ids(pool: &base_pool, level: level) -> HashSet<u64> {
  pool
    .on_disk()
    .iter()
    .flat_map(|base| {
      let conn = crate::database::store::open_base_readonly(&base.sqlite);
      super::repository::relation_ids_at_level(&conn, level)
    })
    .collect()
}

pub(super) fn known_way_ids(pool: &base_pool, level: level) -> HashSet<u64> {
  pool
    .on_disk()
    .iter()
    .flat_map(|base| {
      let conn = crate::database::store::open_base_readonly(&base.sqlite);
      super::repository::way_ids_at_level(&conn, level)
    })
    .collect()
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
  finished { kept: u64, dropped: u64 },
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
    pool: &mut base_pool,
    mut on_event: impl FnMut(extract_event),
  ) {
    for stage in Self::stages_of(level) {
      let mut emit = |step: extract_step| on_event(extract_event { stage, step });
      emit(extract_step::started);
      let written = match stage.source {
        source::relations => {
          let found = crate::osm_relation::repository::all_ids_by_admin_level(conn, level);
          let known = known_relation_ids(pool, level);
          let pending: Vec<u64> = found
            .iter()
            .copied()
            .filter(|id| !known.contains(id))
            .collect();
          emit(extract_step::candidates {
            found: found.len() as u64,
            pending: pending.len() as u64,
          });
          relations::run_with_ids(conn, pending, level, opts, pool, |report| {
            emit(extract_step::progress(report))
          })
        }
        source::place_ways => place_ways::run(conn, opts, pool, |report| {
          emit(extract_step::progress(report))
        }),
        source::streets => streets::run(conn, opts, pool, |report| {
          emit(extract_step::progress(report))
        }),
      };
      emit(extract_step::finished {
        kept: written.kept,
        dropped: written.dropped,
      });
    }
  }
}
