use rusqlite::Connection;

use super::entity::osm_node_row;
use crate::domain::table;

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS osm_data.osm_nodes (
    id INTEGER PRIMARY KEY,
    osm_pbf_chunk_id INTEGER,
    payload BLOB NOT NULL
  );
";

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS osm_data.osm_nodes_search_by_chunk ON osm_nodes(osm_pbf_chunk_id);
";

const SQL_DROP: &str = "DROP TABLE IF EXISTS osm_data.osm_nodes;";

pub struct osm_nodes;

impl table for osm_nodes {
  const CREATE: &str = SQL_CREATE;
  const INDEXES: &str = SQL_CREATE_INDEXES;
}

pub(crate) fn drop_table(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP)
    .expect("failed to drop osm_nodes");
}

pub fn insert_rows(conn: &Connection, rows: &[osm_node_row]) {
  const SQL_INSERT_HEAD: &str = "
    INSERT OR IGNORE INTO osm_data.osm_nodes (
      id,
      osm_pbf_chunk_id,
      payload
    ) VALUES
  ";

  let params: Vec<[&dyn rusqlite::ToSql; 3]> = rows
    .iter()
    .map(|row| [&row.id as &dyn rusqlite::ToSql, &row.osm_pbf_chunk_id, &row.payload])
    .collect();
  crate::database::insert_in_chunks(conn, SQL_INSERT_HEAD, &params);
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod tests;
