use serde::Serialize;
use utoipa::ToSchema;

use crate::admin_level_hierarchy::tantivy_index;

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

// one entry per base; is_ok is every service of every base available
pub(crate) fn build<'a>(
  bases: impl Iterator<Item = (String, &'a rusqlite::Connection, Option<&'a tantivy_index>)>,
) -> status_output {
  let databases: Vec<database_status> = bases
    .map(|(file, conn, index)| database_status {
      file,
      text_to_address: index.is_some(),
      coordinates_to_address: probe_coordinates(conn),
    })
    .collect();
  let is_ok = !databases.is_empty()
    && databases
      .iter()
      .all(|database| database.text_to_address && database.coordinates_to_address);
  status_output { is_ok, databases }
}

// structural probe: `prepare` compiles the table reference, so this is Ok iff admin_levels_rtree
// exists — the distinguishing dependency of the reverse-geocoding path. no row i/o.
fn probe_coordinates(conn: &rusqlite::Connection) -> bool {
  conn.prepare(SQL_COORDINATES_PROBE).is_ok()
}
