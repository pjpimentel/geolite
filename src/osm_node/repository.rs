use rusqlite::Connection;

use super::entity::osm_node_row;
use crate::database::table;

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS osm_nodes (
    id INTEGER PRIMARY KEY,
    osm_pbf_chunk_id INTEGER,
    payload BLOB NOT NULL
  );
";

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS osm_nodes_search_by_chunk ON osm_nodes(osm_pbf_chunk_id);
";

pub struct osm_nodes;

impl table for osm_nodes {
  const CREATE: &str = SQL_CREATE;
  const INDEXES: &str = SQL_CREATE_INDEXES;
}

pub fn delete_by_file_id(conn: &Connection, file_id: u32) {
  const SQL_DELETE_BY_FILE_ID: &str = "
    DELETE FROM osm_nodes
    WHERE osm_pbf_chunk_id IN (
      SELECT id
      FROM osm_pbf_blob_chunks
      WHERE file_id = ?1
    )
  ";

  conn
    .execute(SQL_DELETE_BY_FILE_ID, [file_id])
    .expect("failed to delete the nodes of a file");
}

pub fn insert_rows(conn: &Connection, rows: &[osm_node_row]) {
  const SQL_INSERT_HEAD: &str = "
    INSERT OR IGNORE INTO osm_nodes (
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
