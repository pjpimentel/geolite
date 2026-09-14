use super::scale::level;
use crate::domain::osm_way::way_filter;

#[derive(Clone, Copy)]
pub struct extraction_rules {
  pub level: level,
  pub include: &'static [way_filter],
  pub exclude: &'static [way_filter],
}

fn default_include(level: level) -> &'static [way_filter] {
  match level {
    level::neighborhood => &[
      way_filter::include_place_neighbourhood,
      way_filter::include_place_suburb,
    ],
    _ => &[],
  }
}

fn default_exclude(level: level) -> &'static [way_filter] {
  match level {
    level::street => &[
      way_filter::exclude_place_neighbourhood,
      way_filter::exclude_place_suburb,
      way_filter::exclude_leisure_park,
      way_filter::exclude_building,
      way_filter::exclude_waterway,
    ],
    _ => &[],
  }
}

pub(super) fn resolve_rules(
  level: level,
  overrides: &[extraction_rules],
) -> (&'static [way_filter], &'static [way_filter]) {
  if let Some(r) = overrides.iter().find(|r| r.level == level) {
    return (r.include, r.exclude);
  }
  (default_include(level), default_exclude(level))
}

#[cfg(test)]
#[path = "rules.test.rs"]
mod tests;
