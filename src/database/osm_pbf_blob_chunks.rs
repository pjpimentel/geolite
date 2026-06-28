use rusqlite::Connection;

#[derive(Clone, Copy)]
pub enum chunk_type {
  header = 0,
  data = 1,
}
impl rusqlite::types::FromSql for chunk_type {
  fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
    match value.as_i64()? {
      0 => Ok(chunk_type::header),
      1 => Ok(chunk_type::data),
      other => Err(rusqlite::types::FromSqlError::OutOfRange(other)),
    }
  }
}

impl rusqlite::types::ToSql for chunk_type {
  fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
    Ok((*self as i8).into())
  }
}

pub struct osm_pbf_blob_chunk {
  pub id: u32,
  pub file_id: u32,
  pub first_byte: u64,
  pub chunk_size: u64,
  pub data_first_byte: u64,
  pub data_size: u64,
  pub chunk_type: chunk_type,
}

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS osm_data.osm_pbf_blob_chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    first_byte INTEGER NOT NULL,
    chunk_size INTEGER NOT NULL,
    data_first_byte INTEGER NOT NULL,
    data_size INTEGER NOT NULL,
    chunk_type INTEGER NOT NULL,
    UNIQUE (file_id, first_byte)
  );
";

const SQL_DROP: &str = "DROP TABLE IF EXISTS osm_data.osm_pbf_blob_chunks;";

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS osm_data.blob_chunks_search_by_file_and_type
    ON osm_pbf_blob_chunks(file_id, chunk_type);
";

impl_table_ops!(pub(super), SQL_CREATE, SQL_DROP);

pub fn create_indexes(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE_INDEXES)
    .expect("failed to create blob_chunks indexes");
}

pub fn batch_insert(conn: &Connection, chunks: &[osm_pbf_blob_chunk]) {
  let tx = conn
    .unchecked_transaction()
    .expect("failed to begin transaction");
  {
    let mut stmt = tx
      .prepare(
        "INSERT INTO osm_data.osm_pbf_blob_chunks
           (file_id, first_byte, chunk_size, data_first_byte, data_size, chunk_type)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6)
           ON CONFLICT (file_id, first_byte) DO UPDATE SET
             chunk_size = excluded.chunk_size,
             data_first_byte = excluded.data_first_byte,
             data_size = excluded.data_size,
             chunk_type = excluded.chunk_type",
      )
      .expect("failed to prepare statement");
    for chunk in chunks {
      stmt
        .execute(rusqlite::params![
          chunk.file_id,
          chunk.first_byte,
          chunk.chunk_size,
          chunk.data_first_byte,
          chunk.data_size,
          chunk.chunk_type,
        ])
        .expect("failed to insert blob chunk");
    }
  }
  tx.commit().expect("failed to commit");
}

pub fn count_by_file_id(conn: &Connection, file_id: u32) -> i64 {
  conn
    .query_row(
      "SELECT COUNT(*) FROM osm_data.osm_pbf_blob_chunks WHERE file_id = ?1",
      rusqlite::params![file_id],
      |row| row.get(0),
    )
    .expect("failed to count blob chunks")
}

pub fn get_header_chunk(conn: &Connection, file_id: u32) -> Option<osm_pbf_blob_chunk> {
  let mut stmt = conn
    .prepare(
      "SELECT id, first_byte, chunk_size, data_first_byte, data_size, chunk_type
       FROM osm_data.osm_pbf_blob_chunks
       WHERE chunk_type = 0 AND file_id = ?1
       LIMIT 1",
    )
    .expect("failed to prepare");

  stmt
    .query_row(rusqlite::params![file_id], |row| {
      Ok(osm_pbf_blob_chunk {
        id: row.get(0).unwrap(),
        file_id,
        first_byte: row.get(1).unwrap(),
        chunk_size: row.get(2).unwrap(),
        data_first_byte: row.get(3).unwrap(),
        data_size: row.get(4).unwrap(),
        chunk_type: row.get(5).unwrap(),
      })
    })
    .ok()
}

pub fn get_data_chunks(conn: &Connection, file_id: u32) -> Vec<osm_pbf_blob_chunk> {
  let mut statement = conn
    .prepare(
      "SELECT id, first_byte, chunk_size, data_first_byte, data_size, chunk_type
       FROM osm_data.osm_pbf_blob_chunks
       WHERE file_id = ?1
       ORDER BY first_byte ASC",
    )
    .expect("failed to prepare");

  statement
    .query_map(rusqlite::params![file_id], |row| {
      Ok(osm_pbf_blob_chunk {
        id: row.get(0).unwrap(),
        file_id,
        first_byte: row.get(1).unwrap(),
        chunk_size: row.get(2).unwrap(),
        data_first_byte: row.get(3).unwrap(),
        data_size: row.get(4).unwrap(),
        chunk_type: row.get(5).unwrap(),
      })
    })
    .expect("failed to query")
    .collect::<Result<Vec<_>, _>>()
    .expect("failed to collect")
}
