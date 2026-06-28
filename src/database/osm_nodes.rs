use rusqlite::Connection;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

impl ToSql for crate::extract::osm_data::osm_nodes::osm_node {
  fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
    let json = serde_json::to_string(self)
      .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    Ok(ToSqlOutput::Owned(rusqlite::types::Value::Text(json)))
  }
}

impl FromSql for crate::extract::osm_data::osm_nodes::osm_node {
  fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
    let text = value.as_str()?;
    serde_json::from_str(text).map_err(|e| FromSqlError::Other(Box::new(e)))
  }
}

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

impl_table_ops!(pub(super), SQL_CREATE, SQL_DROP);

pub fn create_indexes(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE_INDEXES)
    .expect("failed to create osm_nodes indexes");
}

pub struct osm_node_row {
  pub id: u64,
  pub osm_pbf_chunk_id: u32,
  // pre-encoded JSONB binary (sqlite jsonb format) — bound diretamente como BLOB
  pub payload: Vec<u8>,
}

pub(crate) fn insert_rows(conn: &Connection, rows: &[osm_node_row]) {
  if rows.is_empty() {
    return;
  }
  for chunk in rows.chunks(INSERT_CHUNK_SIZE) {
    let sql = build_multi_insert_sql(chunk.len());
    let mut stmt = conn
      .prepare_cached(&sql)
      .expect("failed to prepare statement");
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
#[path = "osm_nodes.test.rs"]
mod tests;
