use serde::Serialize;
use utoipa::ToSchema;

use crate::index::admin_levels_hierarchy_tantivy::tantivy_index;

const SQL_COORDINATES_PROBE: &str = "
  SELECT 1
  FROM admin_levels_rtree
  LIMIT 1
";

#[derive(Serialize, ToSchema)]
pub(crate) struct status_output {
  pub(crate) is_ok: bool,
  pub(crate) databases: Vec<database_status>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct database_status {
  pub(crate) file: String,
  pub(crate) text_to_address: bool,
  pub(crate) coordinates_to_address: bool,
}

pub(crate) fn build(
  file: &str,
  conn: &rusqlite::Connection,
  index: Option<&tantivy_index>,
) -> status_output {
  // text_to_address needs the tantivy index loaded; coordinates_to_address needs the rtree path.
  assemble(file, index.is_some(), probe_coordinates(conn))
}

// pure assembly, separated so the is_ok truth table is testable without a real tantivy index.
fn assemble(file: &str, text_to_address: bool, coordinates_to_address: bool) -> status_output {
  let database = database_status {
    file: file.to_string(),
    text_to_address,
    coordinates_to_address,
  };
  // is_ok = every service of every database available; with one database this is just its two flags
  // (it generalizes to an AND over all entries once the attach multi-database feature lands).
  let is_ok = database.text_to_address && database.coordinates_to_address;
  status_output {
    is_ok,
    databases: vec![database],
  }
}

// structural probe: `prepare` compiles the table reference, so this is Ok iff admin_levels_rtree
// exists — the distinguishing dependency of the reverse-geocoding path. no row i/o.
fn probe_coordinates(conn: &rusqlite::Connection) -> bool {
  conn.prepare(SQL_COORDINATES_PROBE).is_ok()
}

#[cfg(test)]
#[path = "status.test.rs"]
mod tests;
