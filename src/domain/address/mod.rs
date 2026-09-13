mod coordinates;
mod entity;
mod filter;
mod house_number;
mod input;
mod label;
mod text;

#[cfg(test)]
#[path = "fixtures.test.rs"]
pub(crate) mod fixtures;

use crate::domain::admin_level::level;
use crate::domain::admin_level_hierarchy::tantivy_index;
use crate::domain::house_number::house_number_policy;

pub use entity::{
  admin_level, house_number_match, query_house_number, query_match, query_match_attributes,
  query_output, query_service,
};
pub use filter::bounding_geometry;
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

pub struct address<'a> {
  conn: &'a rusqlite::Connection,
  index: Option<&'a tantivy_index>,
  house_numbers: &'a house_number_policy,
}

impl<'a> address<'a> {
  pub fn open(
    conn: &'a rusqlite::Connection,
    index: Option<&'a tantivy_index>,
    house_numbers: &'a house_number_policy,
  ) -> Self {
    address {
      conn,
      index,
      house_numbers,
    }
  }

  pub fn query_by_text(&self, text: &str, opts: &query_opts) -> query_output {
    let index = self
      .index
      .expect("can not query because index is unavailable");
    text::run(self.conn, self.house_numbers, index, text, opts)
  }

  pub fn query_by_coordinates(&self, latitude: f64, longitude: f64, opts: &query_opts) -> query_output {
    coordinates::run(self.conn, latitude, longitude, opts)
  }
}
