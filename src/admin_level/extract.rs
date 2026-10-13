use geo::Geometry;
use rusqlite::Connection;

use super::country_index::country_index;
use super::entity::admin_level;
use super::scale::level;
use super::{place_ways, relations, streets};
use crate::progress_report;

pub struct extract_opts<'a> {
  pub threads: usize,
  pub name_priority: Option<&'a [&'a str]>,
  pub name_tags: &'a [&'a str],
  pub countries: &'a country_index,
}

pub(super) struct identity {
  pub(super) name: String,
  pub(super) country_iso_code: Option<String>,
}

pub(super) fn identity_of(
  opts: &extract_opts,
  level: level,
  own_iso: Option<&str>,
  geometry: &Geometry<f64>,
  names: &[Option<String>],
) -> Option<identity> {
  let preset_iso = opts.countries.country_of(geometry);
  let preset = crate::presets::for_country(preset_iso).extract_osm_admin_levels;
  // a country without a preset keeps the code its relation carries, so the ladder still names it
  let iso: Option<String> = preset_iso
    .or(if level == level::country { own_iso } else { None })
    .map(str::to_owned);
  if !preset.admin_levels.contains(&level) {
    return None;
  }
  let priority = opts.name_priority.unwrap_or(preset.name_priority);
  let name = name_by_priority(names, opts.name_tags, priority)?;
  Some(identity {
    name,
    country_iso_code: iso,
  })
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
  finished { kept: u64 },
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
      let kept = match stage.source {
        source::relations => {
          let found =
            crate::osm_relation::repository::all_ids_by_admin_level(conn, level).len() as u64;
          let pending =
            crate::osm_relation::repository::remaining_ids_by_admin_level(conn, level);
          emit(extract_step::candidates {
            found,
            pending: pending.len() as u64,
          });
          relations::run_with_ids(conn, pending, level, opts, |report| {
            emit(extract_step::progress(report))
          })
        }
        source::place_ways => {
          place_ways::run(conn, opts, |report| emit(extract_step::progress(report)))
        }
        source::streets => streets::run(conn, opts, |report| emit(extract_step::progress(report))),
      };
      emit(extract_step::finished { kept });
    }
  }
}
