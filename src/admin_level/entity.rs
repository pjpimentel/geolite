use super::geometry::admin_geometry;
use super::id::admin_level_id;
use super::scale::level;

pub struct admin_level {
  pub id: admin_level_id,
  pub level: level,
  pub wkb: admin_geometry,
  pub name: String,
  pub country_iso_code: Option<String>,
  pub post_code: Option<String>,
}
