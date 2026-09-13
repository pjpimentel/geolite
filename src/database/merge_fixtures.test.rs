use rusqlite::Connection;

use crate::domain::house_number::fixtures::link;
use crate::domain::house_number::house_number_link;
use crate::domain::house_number::repository::batch_insert_links;
use crate::database::open_write_main;
use crate::domain::admin_level::geometry::admin_geometry;
use crate::domain::admin_level::repository::batch_upsert;
use crate::domain::admin_level::{admin_level as admin_levels_row, level};
use geo::{Coord, Geometry, LineString};

fn make_geometry() -> admin_geometry {
  Geometry::LineString(LineString(vec![
    Coord { x: 0.0, y: 0.0 },
    Coord { x: 0.001, y: 0.001 },
  ]))
  .into()
}

pub(crate) fn make_way(way_id: u64) -> admin_levels_row {
  admin_levels_row {
    relation_id: None,
    way_id: Some(way_id),
    level: level::street,
    wkb: make_geometry(),
    name: format!("way_{way_id}"),
    country_iso_code: None,
    post_code: None,
  }
}

pub(crate) fn make_house(node_id: u64, admin_level_id: i64, number: &str) -> house_number_link {
  link(node_id, admin_level_id, number, 0.0, 0.0)
}

// each test gets its own on-disk database files so they can be ATTACHED by path; sqlite cannot
// attach a :memory: database of another connection. the tag is unique across every merge test.
pub(crate) fn temp_path(tag: &str) -> String {
  let dir = std::env::temp_dir();
  let path = dir.join(format!("geolite_merge_{tag}_{}.sqlite3", std::process::id()));
  for suffix in ["", "-wal", "-shm"] {
    let _ = std::fs::remove_file(format!("{}{suffix}", path.to_string_lossy()));
  }
  path.to_string_lossy().into_owned()
}

pub(crate) fn cleanup(path: &str) {
  for suffix in ["", "-wal", "-shm"] {
    let _ = std::fs::remove_file(format!("{path}{suffix}"));
  }
}

// builds a source database file and checkpoints the WAL so it can be attached read-only.
pub(crate) fn build_source(path: &str, admins: &[admin_levels_row], houses: &[house_number_link]) {
  let conn = open_write_main(path);
  batch_upsert(&conn, admins);
  batch_insert_links(&conn, houses);
  conn
    .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
    .expect("failed to checkpoint source");
  drop(conn);
}

pub(crate) fn count(conn: &Connection, sql: &str) -> i64 {
  conn
    .query_row(sql, [], |row| row.get(0))
    .expect("failed to count")
}
