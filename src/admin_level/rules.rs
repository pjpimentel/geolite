use super::scale::level;
use crate::osm_way::way_filter;

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

pub(super) fn rules_of(level: level) -> (&'static [way_filter], &'static [way_filter]) {
  (default_include(level), default_exclude(level))
}
