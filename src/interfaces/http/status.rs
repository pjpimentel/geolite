use serde::Serialize;
use utoipa::ToSchema;

use crate::address::database_status;
use crate::admin_level_hierarchy::tantivy_index;

#[derive(Serialize, ToSchema)]
pub(crate) struct status_output {
  pub(crate) is_ok: bool,
  pub(crate) databases: Vec<database_status>,
}

// one entry per base; is_ok is every service of every base available
pub(crate) fn build<'a>(
  bases: impl Iterator<Item = (String, &'a rusqlite::Connection, Option<&'a tantivy_index>)>,
) -> status_output {
  let databases: Vec<database_status> = bases
    .map(|(file, conn, index)| database_status::of(&file, conn, index))
    .collect();
  let is_ok = !databases.is_empty()
    && databases
      .iter()
      .all(|database| database.text_to_address && database.coordinates_to_address);
  status_output { is_ok, databases }
}
