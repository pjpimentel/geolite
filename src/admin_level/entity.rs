use super::geometry::admin_geometry;
use super::scale::level;

pub struct admin_level {
  pub relation_id: Option<u64>,
  pub way_id: Option<u64>,
  pub level: level,
  pub wkb: admin_geometry,
  pub name: String,
  pub country_iso_code: Option<String>,
  pub post_code: Option<String>,
}
