use std::path::Path;

use rusqlite::{Connection, OpenFlags};

// bumped whenever the on-disk schema or the layout of the data path changes in a way that makes
// builds incompatible; stamped into every writable file via PRAGMA user_version, and refused on
// open when another version is found
pub const SCHEMA_VERSION: u32 = 4;

pub mod jsonb;
pub mod store;

pub(crate) trait table {
  const CREATE: &str;
  const INDEXES: &str;

  fn create_table(conn: &rusqlite::Connection) {
    conn
      .execute_batch(Self::CREATE)
      .expect("failed to create table");
  }

  fn create_indexes(conn: &rusqlite::Connection) {
    conn
      .execute_batch(Self::INDEXES)
      .expect("failed to create indexes");
  }
}

pub(crate) fn placeholders_for(count: usize) -> String {
  vec!["?"; count].join(",")
}

pub(crate) fn query_by_ids<T>(
  conn: &Connection,
  sql: &str,
  ids: impl IntoIterator<Item = i64>,
  row_of: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Vec<T> {
  let params: Vec<rusqlite::types::Value> = ids
    .into_iter()
    .map(rusqlite::types::Value::Integer)
    .collect();
  let mut stmt = conn
    .prepare(sql)
    .expect("failed to prepare query by ids");
  stmt
    .query_map(rusqlite::params_from_iter(params.iter()), row_of)
    .expect("failed to query by ids")
    .map(|r| r.expect("failed to read row"))
    .collect()
}

const INSERT_CHUNK_SIZE: usize = 10_000;

pub(crate) fn insert_in_chunks<const N: usize>(
  conn: &Connection,
  sql_head: &str,
  rows: &[[&dyn rusqlite::ToSql; N]],
) {
  // SQLITE_MAX_VARIABLE_NUMBER is 32766 on the bundled build: a wider row would overflow it
  const { assert!(N * INSERT_CHUNK_SIZE <= 32_766) };
  for chunk in rows.chunks(INSERT_CHUNK_SIZE) {
    let sql = multi_insert_sql(sql_head, N, chunk.len());
    let mut stmt = conn
      .prepare_cached(&sql)
      .expect("failed to prepare chunked insert");
    stmt
      .execute(rusqlite::params_from_iter(chunk.iter().flatten()))
      .expect("failed to insert chunk");
  }
}

fn multi_insert_sql(sql_head: &str, columns: usize, rows: usize) -> String {
  use std::fmt::Write;
  let mut sql = String::with_capacity(sql_head.len() + rows * (columns * 8 + 4));
  sql.push_str(sql_head);
  for row in 0..rows {
    if row > 0 {
      sql.push_str(",\n");
    }
    sql.push_str("  (");
    for column in 0..columns {
      if column > 0 {
        sql.push_str(", ");
      }
      write!(sql, "?{}", row * columns + column + 1).unwrap();
    }
    sql.push(')');
  }
  sql
}

pub fn compact(conn: &Connection) -> (u64, u64) {
  const SQL_PAGE_SIZE: &str = "PRAGMA page_size";
  const SQL_PAGE_COUNT: &str = "PRAGMA page_count";
  const SQL_COMPACT: &str = "
    ANALYZE;
    PRAGMA optimize;
    PRAGMA wal_checkpoint(TRUNCATE);
    VACUUM;
  ";

  let page_size: u64 = conn
    .query_row(SQL_PAGE_SIZE, [], |r| r.get(0))
    .unwrap_or(4096);
  let pages_before: u64 = conn
    .query_row(SQL_PAGE_COUNT, [], |r| r.get(0))
    .unwrap_or(0);
  conn
    .execute_batch(SQL_COMPACT)
    .expect("failed to compact sqlite file");
  let pages_after: u64 = conn
    .query_row(SQL_PAGE_COUNT, [], |r| r.get(0))
    .unwrap_or(0);
  (page_size * pages_before, page_size * pages_after)
}

pub(crate) fn has_column(conn: &Connection, schema: &str, table: &str, column: &str) -> bool {
  const SQL_HAS_COLUMN: &str = "
    SELECT COUNT(*)
    FROM PRAGMA_TABLE_INFO(?1, ?2)
    WHERE name = ?3
  ";

  conn
    .query_row(
      SQL_HAS_COLUMN,
      rusqlite::params![table, schema, column],
      |row| row.get::<_, i64>(0),
    )
    .expect("failed to read table info")
    > 0
}

// a writable file of the data path, the osm_data or a base: created on first open, stamped with
// the schema version and refused when stamped with another
pub(crate) fn open_writable(path: &str) -> Connection {
  // create_dir_all races between concurrent geolite processes sharing an ancestor directory:
  // std can return AlreadyExists spuriously, so a parent that already exists is success.
  if let Some(parent) = std::path::Path::new(path).parent()
    && !parent.as_os_str().is_empty()
    && let Err(error) = std::fs::create_dir_all(parent)
    && !parent.is_dir()
  {
    panic!("failed to create sqlite parent dir: {error}");
  }
  if is_sealed(path) {
    panic!("{path} is sealed: a base built once is final \u{2014} remove it to build it again");
  }
  let conn = Connection::open(path).expect("failed to open sqlite");
  let stamped = require_schema_version(&conn, path);
  conn
    .execute_batch(
      "PRAGMA journal_mode=WAL;
       PRAGMA synchronous=NORMAL;
       PRAGMA cache_size=-32768;
       PRAGMA temp_store=MEMORY;",
    )
    .expect("failed to set pragmas");
  // a stamp already in place is not written again: the write would fail on the lock of a second
  // process writing the file instead of letting that process be named
  if stamped != SCHEMA_VERSION {
    conn
      .pragma_update(None, "user_version", SCHEMA_VERSION)
      .expect("failed to set user_version");
  }
  conn
}

fn require_schema_version(conn: &Connection, path: &str) -> u32 {
  let stamped: u32 = conn
    .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
    .map(|v| v as u32)
    .expect("failed to read user_version");
  if stamped != 0 && stamped != SCHEMA_VERSION {
    panic!(
      "incompatible schema version on {path}: found {stamped}, expected {SCHEMA_VERSION} \u{2014} rebuild required"
    );
  }
  stamped
}

// what a build leaves behind is read-only at the os level as well, a file or a directory with
// everything inside it, and stays so: nothing lifts it. tantivy takes its meta lock to read, so
// the lock files keep their write bit
pub fn seal_path(path: impl AsRef<Path>) {
  seal(path.as_ref());
}

pub fn is_sealed(path: impl AsRef<Path>) -> bool {
  std::fs::metadata(path).is_ok_and(|metadata| metadata.permissions().readonly())
}

#[cfg(unix)]
fn seal(path: &Path) {
  use std::os::unix::fs::PermissionsExt;

  let Ok(metadata) = std::fs::metadata(path) else {
    return;
  };
  if metadata.is_dir() {
    for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
      seal(&entry.path());
    }
  }
  if path.extension().is_some_and(|ext| ext == "lock") {
    return;
  }
  let mode = if metadata.is_dir() { 0o555 } else { 0o444 };
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    .unwrap_or_else(|e| panic!("failed to seal {}: {e}", path.display()));
}

#[cfg(not(unix))]
fn seal(_path: &Path) {}

// a read of a file another connection of the same process may be writing: it follows the wal
pub fn open_readonly(path: &str) -> Connection {
  let conn = Connection::open_with_flags(
    path,
    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
  )
  .expect("failed to open sqlite readonly");
  set_reader_pragmas(&conn);
  conn
}

// immutable skips the locks and the wal-index, so a reader never creates a `-wal` or a `-shm`
// beside a base and a base on read-only media opens; a base written while it is read is seen as
// it was until the reader opens it again
pub fn open_immutable(path: &str) -> Connection {
  let conn = Connection::open_with_flags(
    immutable_uri(path),
    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI,
  )
  .expect("failed to open sqlite immutable");
  set_reader_pragmas(&conn);
  conn
}

fn set_reader_pragmas(conn: &Connection) {
  conn
    .execute_batch(
      "PRAGMA cache_size=-262144;
       PRAGMA mmap_size=2147483648;
       PRAGMA temp_store=MEMORY;
       PRAGMA query_only=1;",
    )
    .expect("failed to set pragmas");
}

fn immutable_uri(path: &str) -> String {
  let mut escaped = String::with_capacity(path.len());
  for c in path.chars() {
    match c {
      '%' => escaped.push_str("%25"),
      '?' => escaped.push_str("%3F"),
      '#' => escaped.push_str("%23"),
      c => escaped.push(c),
    }
  }
  format!("file:{escaped}?mode=ro&immutable=1")
}
