use rusqlite::{Connection, OpenFlags};

use crate::domain::table;

#[macro_export]
macro_rules! impl_table_ops {
  ($vis:vis, $create:expr, $drop:expr) => {
    $vis fn create_table(conn: &rusqlite::Connection) {
      conn.execute_batch($create).expect("failed to create table");
    }
    #[allow(dead_code)]
    $vis fn drop_table(conn: &rusqlite::Connection) {
      conn.execute_batch($drop).expect("failed to drop table");
    }
    // TODO: create indexes
  };
}

// bumped whenever the on-disk schema changes in a way that makes builds incompatible;
// stamped into every writable database via PRAGMA user_version. open_write_main refuses a
// database stamped with another version, and `geolite merge` refuses to combine one.
pub const SCHEMA_VERSION: u32 = 2;

pub mod admin_levels;
pub mod admin_levels_hierarchy;
pub mod house_numbers;
pub mod jsonb;
pub mod merge;
pub mod osm_nodes;
pub mod osm_relations;
pub mod osm_ways;

pub(crate) fn placeholders_for(ids: &[u64]) -> String {
  ids.iter().map(|_| "?").collect::<Vec<_>>().join(",")
}

pub(crate) fn query_by_ids<T>(
  conn: &Connection,
  sql: &str,
  ids: &[u64],
  row_of: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Vec<T> {
  let params: Vec<rusqlite::types::Value> = ids
    .iter()
    .map(|&id| rusqlite::types::Value::Integer(id as i64))
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

pub fn osm_data_path(main_path: &str) -> String {
  if main_path == ":memory:" {
    return ":memory:".to_string();
  }
  let p = std::path::Path::new(main_path);
  let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("database");
  let parent = p.parent().unwrap_or(std::path::Path::new(""));
  parent
    .join(format!("{stem}.osm_data.sqlite3"))
    .to_string_lossy()
    .into_owned()
}

fn remove_osm_data_files(main_path: &str) {
  if main_path == ":memory:" {
    return;
  }
  let sibling = osm_data_path(main_path);
  for suffix in ["", "-wal", "-shm"] {
    let p = format!("{sibling}{suffix}");
    if std::path::Path::new(&p).exists() {
      std::fs::remove_file(&p).expect("failed to remove osm_data sibling");
    }
  }
}

pub fn destroy_data(
  path: &str,
  osm_pbf_blob_chunks: bool,
  osm_data: bool,
  admin_levels: bool,
  house_numbers: bool,
) {
  if osm_pbf_blob_chunks || osm_data {
    remove_osm_data_files(path);
  }
  let conn = open_write_main(path);
  if house_numbers {
    house_numbers::drop_table(&conn);
  }
  if admin_levels {
    admin_levels_hierarchy::drop_table(&conn);
    admin_levels::drop_rtree(&conn);
    admin_levels::drop_table(&conn);
    house_numbers::drop_table(&conn);
  }
  conn.execute_batch("VACUUM;").expect("failed to vacuum");
}

pub fn open_write_main(path: &str) -> Connection {
  if let Some(parent) = std::path::Path::new(path).parent()
    && !parent.as_os_str().is_empty()
  {
    std::fs::create_dir_all(parent).expect("failed to create sqlite parent dir");
  }
  let conn = Connection::open(path).expect("failed to open sqlite");
  let stamped: u32 = conn
    .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
    .map(|v| v as u32)
    .expect("failed to read user_version");
  if stamped != 0 && stamped != SCHEMA_VERSION {
    panic!(
      "incompatible schema version on {path}: found {stamped}, expected {SCHEMA_VERSION} \u{2014} rebuild required"
    );
  }
  conn
    .execute_batch(
      "PRAGMA journal_mode=WAL;
       PRAGMA synchronous=NORMAL;
       PRAGMA cache_size=-32768;
       PRAGMA temp_store=MEMORY;",
    )
    .expect("failed to set pragmas");
  conn
    .pragma_update(None, "user_version", SCHEMA_VERSION)
    .expect("failed to set user_version");
  crate::domain::osm_pbf_file::osm_pbf_files::create_table(&conn);
  admin_levels::create_table(&conn);
  admin_levels_hierarchy::create_table(&conn);
  admin_levels::create_rtree(&conn);
  house_numbers::create_table(&conn);
  conn
}

pub fn open_write(path: &str) -> Connection {
  let conn = open_write_main(path);
  let osm_data = osm_data_path(path);
  conn
    .execute("ATTACH DATABASE ?1 AS osm_data", [&osm_data])
    .expect("failed to attach osm_data");
  conn
    .execute_batch(
      "PRAGMA osm_data.journal_mode=WAL;
       PRAGMA osm_data.synchronous=NORMAL;
       PRAGMA osm_data.cache_size=-32768;",
    )
    .expect("failed to set osm_data pragmas");
  crate::domain::osm_pbf_file::osm_pbf_blob_chunks::create_table(&conn);
  osm_nodes::create_table(&conn);
  osm_ways::create_table(&conn);
  osm_relations::create_table(&conn);
  conn
}

pub fn open_readonly(path: &str) -> Connection {
  let conn = Connection::open_with_flags(
    path,
    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
  )
  .expect("failed to open sqlite readonly");
  conn
    .execute_batch(
      "PRAGMA cache_size=-262144;
       PRAGMA mmap_size=2147483648;
       PRAGMA temp_store=MEMORY;
       PRAGMA query_only=1;",
    )
    .expect("failed to set pragmas");
  let osm_data = osm_data_path(path);
  if std::path::Path::new(&osm_data).exists() {
    let canonical = std::fs::canonicalize(&osm_data)
      .map(|p| p.to_string_lossy().into_owned())
      .unwrap_or(osm_data);
    let uri = format!("file:{canonical}?mode=ro");
    conn
      .execute("ATTACH DATABASE ?1 AS osm_data", [&uri])
      .expect("failed to attach osm_data readonly");
  }
  conn
}

// reads the schema version stamped into a database (PRAGMA user_version). databases built before
// versioning existed return 0. used by `geolite merge` to reject incompatible builds before mutating.
pub fn read_user_version(path: &str) -> u32 {
  let conn = open_readonly(path);
  conn
    .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
    .map(|v| v as u32)
    .unwrap_or(0)
}
