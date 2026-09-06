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

pub(crate) fn create_table(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE)
    .expect("failed to create osm_pbf_blob_chunks");
}

#[allow(dead_code)]
pub(crate) fn drop_table(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP)
    .expect("failed to drop osm_pbf_blob_chunks");
}

pub fn create_indexes(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE_INDEXES)
    .expect("failed to create blob_chunks indexes");
}

const SQL_COUNT_BY_FILE_ID: &str = "
  SELECT COUNT(*)
  FROM osm_data.osm_pbf_blob_chunks
  WHERE file_id = ?1
";

pub fn count_by_file_id(conn: &Connection, file_id: u32) -> i64 {
  conn
    .query_row(SQL_COUNT_BY_FILE_ID, rusqlite::params![file_id], |row| {
      row.get(0)
    })
    .expect("failed to count blob chunks")
}

const SQL_HEADER_CHUNK: &str = "
  SELECT
    id,
    first_byte,
    chunk_size,
    data_first_byte,
    data_size,
    chunk_type
  FROM osm_data.osm_pbf_blob_chunks
  WHERE chunk_type = 0
  AND file_id = ?1
  LIMIT 1
";

pub fn get_header_chunk(conn: &Connection, file_id: u32) -> Option<osm_pbf_blob_chunk> {
  let mut stmt = conn
    .prepare(SQL_HEADER_CHUNK)
    .expect("failed to prepare header chunk query");

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

const SQL_DATA_CHUNKS: &str = "
  SELECT
    id,
    first_byte,
    chunk_size,
    data_first_byte,
    data_size,
    chunk_type
  FROM osm_data.osm_pbf_blob_chunks
  WHERE file_id = ?1
  ORDER BY first_byte ASC
";

pub fn get_data_chunks(conn: &Connection, file_id: u32) -> Vec<osm_pbf_blob_chunk> {
  let mut statement = conn
    .prepare(SQL_DATA_CHUNKS)
    .expect("failed to prepare data chunks query");

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
    .expect("failed to query blob chunks")
    .collect::<Result<Vec<_>, _>>()
    .expect("failed to collect blob chunks")
}

const SQL_INSERT: &str = "
  INSERT INTO osm_data.osm_pbf_blob_chunks (
    file_id,
    first_byte,
    chunk_size,
    data_first_byte,
    data_size,
    chunk_type
  ) VALUES (
    ?1,
    ?2,
    ?3,
    ?4,
    ?5,
    ?6
  )
  ON CONFLICT (file_id, first_byte) DO UPDATE SET
    chunk_size = excluded.chunk_size,
    data_first_byte = excluded.data_first_byte,
    data_size = excluded.data_size,
    chunk_type = excluded.chunk_type
";

pub fn batch_insert(conn: &Connection, chunks: &[osm_pbf_blob_chunk]) {
  let tx = conn
    .unchecked_transaction()
    .expect("failed to begin transaction");
  {
    let mut stmt = tx
      .prepare(SQL_INSERT)
      .expect("failed to prepare osm_pbf_blob_chunks insert");
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
