use super::geometry::admin_geometry;
use super::scale::level;

// an admin level as it is written: the osm element it came from, where it sits on the scale, its
// shape and its name. the identity is not a field — it is derived from the osm element by
// `admin_level_id`, which is why exactly one of `relation_id` and `way_id` is set.
pub struct admin_level {
  pub relation_id: Option<u64>,
  pub way_id: Option<u64>,
  pub level: level,
  pub wkb: admin_geometry,
  pub name: String,
  // country_iso_code: ISO 3166-1 alpha-2 (2 chars, e.g. 'BR', 'US')
  pub country_iso_code: Option<String>,
  pub post_code: Option<String>,
}

