mod coordinates;
mod entity;
mod fan_out;
mod filter;
mod house_number;
mod input;
mod label;
mod path;
mod text;

use crate::admin_level::geometry::bounding_geometry;
use crate::admin_level::level;
use crate::admin_level_hierarchy::tantivy_index;

pub use entity::{
  admin_level, database_status, query_house_number, query_match, query_match_attributes,
  query_output, query_service,
};
pub(crate) use fan_out::in_parallel;
pub use filter::parse_min_quality;
pub use input::query_input;
pub use label::validate_friendly_name_format;

#[derive(Default)]
pub struct query_opts<'a> {
  pub friendly_name_format: Option<&'a str>,
  pub min_quality: Option<f64>,
  pub bounding: Option<bounding_geometry>,
  pub last_admin_levels: Option<Vec<level>>,
  pub include_wkt: bool,
}

pub fn at_path(conn: &rusqlite::Connection, id: i64, path: &[i64]) -> Option<query_match> {
  path::run(conn, id, path)
}

// a base a question is asked of: its label, its country, its connection, held alone while the
// question runs, and its index when it has one
pub struct base<'a> {
  pub file: &'a str,
  pub iso: &'a str,
  pub conn: &'a mut rusqlite::Connection,
  pub index: Option<&'a tantivy_index>,
}

pub fn country_of_point(latitude: f64, longitude: f64) -> Option<&'static str> {
  crate::admin_level::country_index::country_index::shared().country_at(longitude, latitude)
}

pub fn query_by_text_across(bases: &mut [base], text: &str, opts: &query_opts) -> query_output {
  fan_out::by_text(bases, text, opts)
}

pub fn query_by_coordinates_across(
  bases: &mut [base],
  latitude: f64,
  longitude: f64,
  opts: &query_opts,
) -> query_output {
  fan_out::by_coordinates(bases, latitude, longitude, opts)
}
