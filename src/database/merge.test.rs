use crate::database::admin_levels::{
  admin_geometry, admin_id_kind, admin_levels as admin_levels_row, batch_upsert, pack_admin_id,
};
use crate::database::house_numbers::{batch_insert, house_numbers as house_numbers_row};
use crate::database::open_write_main;
use geo::{Coord, Geometry, LineString};
use rusqlite::Connection;

fn make_geometry() -> admin_geometry {
  Geometry::LineString(LineString(vec![
    Coord { x: 0.0, y: 0.0 },
    Coord { x: 0.001, y: 0.001 },
  ]))
  .into()
}

fn make_way(way_id: u64) -> admin_levels_row {
  admin_levels_row {
    relation_id: None,
    way_id: Some(way_id),
    admin_level: 12,
    wkb: make_geometry(),
    name: format!("way_{way_id}"),
    country_iso_code: None,
    post_code: None,
  }
}

fn make_house(node_id: u64, admin_level_id: i64, number: &str) -> house_numbers_row {
  house_numbers_row {
    node_id,
    admin_level_id,
    number: number.to_string(),
    wkb: make_geometry(),
    strategy: 0,
  }
}

// each test gets its own on-disk database files so they can be ATTACHED by path; sqlite cannot
// attach a :memory: database of another connection.
fn temp_path(tag: &str) -> String {
  let dir = std::env::temp_dir();
  let path = dir.join(format!("geolite_db_merge_{tag}_{}.sqlite3", std::process::id()));
  for suffix in ["", "-wal", "-shm"] {
    let _ = std::fs::remove_file(format!("{}{suffix}", path.to_string_lossy()));
  }
  path.to_string_lossy().into_owned()
}

fn cleanup(path: &str) {
  for suffix in ["", "-wal", "-shm"] {
    let _ = std::fs::remove_file(format!("{path}{suffix}"));
  }
}

// builds a source database file and checkpoints the WAL so it can be attached read-only.
fn build_source(path: &str, admins: &[admin_levels_row], houses: &[house_numbers_row]) {
  let conn = open_write_main(path);
  batch_upsert(&conn, admins);
  batch_insert(&conn, houses);
  conn
    .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
    .expect("failed to checkpoint source");
  drop(conn);
}

fn count(conn: &Connection, sql: &str) -> i64 {
  conn.query_row(sql, [], |row| row.get(0)).expect("failed to count")
}

#[test]
fn _00_merge_combines_admin_levels_without_id_collision() {
  let base_path = temp_path("admins_base");
  let source_path = temp_path("admins_source");

  // base covers ways {1, 2}; source covers ways {2, 3} — way 2 overlaps.
  build_source(&base_path, &[make_way(1), make_way(2)], &[]);
  build_source(&source_path, &[make_way(2), make_way(3)], &[]);

  let conn = open_write_main(&base_path);
  let (admins, _) = super::merge_source(&conn, &source_path);
  assert_eq!(admins, 2, "source contributed two admin_levels rows");

  // ways {1, 2, 3} → exactly three rows; way 2 upserted, not duplicated.
  assert_eq!(count(&conn, "SELECT COUNT(*) FROM admin_levels"), 3);

  let way3_id = pack_admin_id(admin_id_kind::way, 3) as i64;
  assert_eq!(
    count(&conn, &format!("SELECT COUNT(*) FROM admin_levels WHERE id = {way3_id}")),
    1,
    "way 3 from the source is present after merge",
  );

  drop(conn);
  cleanup(&base_path);
  cleanup(&source_path);
}

#[test]
fn _01_merge_house_numbers_dedupes_by_node_id() {
  let base_path = temp_path("houses_base");
  let source_path = temp_path("houses_source");

  // both databases reference admin_level way 1 (id = 2). node 100 overlaps; node 200 is new.
  let way1_id = pack_admin_id(admin_id_kind::way, 1) as i64;
  build_source(&base_path, &[make_way(1)], &[make_house(100, way1_id, "10")]);
  build_source(
    &source_path,
    &[make_way(1)],
    &[make_house(100, way1_id, "999"), make_house(200, way1_id, "20")],
  );

  let conn = open_write_main(&base_path);
  super::merge_source(&conn, &source_path);

  // nodes {100, 200} → exactly two rows; node 100 deduped by its UNIQUE constraint.
  assert_eq!(count(&conn, "SELECT COUNT(*) FROM house_numbers"), 2);

  // INSERT OR IGNORE keeps the base row for node 100 (number "10", not the source's "999").
  let number: String = conn
    .query_row(
      "SELECT number FROM house_numbers WHERE node_id = 100",
      [],
      |row| row.get(0),
    )
    .expect("node 100 must exist");
  assert_eq!(number, "10");

  drop(conn);
  cleanup(&base_path);
  cleanup(&source_path);
}
