use rusqlite::Connection;

use super::entity::osm_node_row;

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS osm_data.osm_nodes (
    id INTEGER PRIMARY KEY,
    osm_pbf_chunk_id INTEGER,
    payload BLOB NOT NULL
  );
";

const SQL_DROP: &str = "DROP TABLE IF EXISTS osm_data.osm_nodes;";

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS osm_data.osm_nodes_search_by_chunk ON osm_nodes(osm_pbf_chunk_id);
";

pub(crate) fn create_table(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE)
    .expect("failed to create osm_nodes");
}

#[allow(dead_code)]
pub(crate) fn drop_table(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP)
    .expect("failed to drop osm_nodes");
}

pub fn create_indexes(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE_INDEXES)
    .expect("failed to create osm_nodes indexes");
}

const INSERT_CHUNK_SIZE: usize = 10_000;

const SQL_INSERT_HEAD: &str = "
  INSERT OR IGNORE INTO osm_data.osm_nodes (
    id,
    osm_pbf_chunk_id,
    payload
  ) VALUES
";

fn build_multi_insert_sql(n: usize) -> String {
  use std::fmt::Write;
  let mut sql = String::with_capacity(SQL_INSERT_HEAD.len() + n * 24);
  sql.push_str(SQL_INSERT_HEAD);
  for i in 0..n {
    if i > 0 {
      sql.push_str(",\n");
    }
    let base = i * 3;
    write!(sql, "  (?{}, ?{}, ?{})", base + 1, base + 2, base + 3).unwrap();
  }
  sql
}

pub fn insert_rows(conn: &Connection, rows: &[osm_node_row]) {
  if rows.is_empty() {
    return;
  }
  for chunk in rows.chunks(INSERT_CHUNK_SIZE) {
    let sql = build_multi_insert_sql(chunk.len());
    let mut stmt = conn
      .prepare_cached(&sql)
      .expect("failed to prepare osm_nodes insert");
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(chunk.len() * 3);
    for row in chunk {
      params.push(&row.id);
      params.push(&row.osm_pbf_chunk_id);
      params.push(&row.payload);
    }
    stmt
      .execute(rusqlite::params_from_iter(params))
      .expect("failed to insert osm_node");
  }
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod tests;
