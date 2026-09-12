use super::scale::level;
use crate::database::osm_ways::filters;

#[derive(Clone, Copy)]
pub struct extraction_rules {
  pub level: level,
  pub include: &'static [filters],
  pub exclude: &'static [filters],
}

fn default_include(level: level) -> &'static [filters] {
  match level {
    level::neighborhood => &[
      filters::include_place_neighbourhood,
      filters::include_place_suburb,
    ],
    _ => &[],
  }
}

fn default_exclude(level: level) -> &'static [filters] {
  match level {
    level::street => &[
      filters::exclude_place_neighbourhood,
      filters::exclude_place_suburb,
      filters::exclude_leisure_park,
      filters::exclude_building,
      filters::exclude_waterway,
    ],
    _ => &[],
  }
}

pub(super) fn resolve_rules(
  level: level,
  overrides: &[extraction_rules],
) -> (&'static [filters], &'static [filters]) {
  if let Some(r) = overrides.iter().find(|r| r.level == level) {
    return (r.include, r.exclude);
  }
  (default_include(level), default_exclude(level))
}

#[cfg(test)]
#[path = "rules.test.rs"]
mod tests;
