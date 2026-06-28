use crate::database::admin_levels::{
  admin_geometry, admin_id_kind, admin_levels as admin_levels_row, batch_upsert, pack_admin_id,
};
use crate::database::house_numbers::{batch_insert, house_numbers as house_numbers_row};
use crate::database::{open_write_main, read_user_version};
use crate::index::admin_levels_hierarchy_tantivy as tantivy;
use crate::presets::DEFAULT;
use crate::query;
use geo::{Coord, Geometry, LineString};
use rusqlite::Connection;
use std::path::Path;

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
  let path = dir.join(format!("geolite_merge_{tag}_{}.sqlite3", std::process::id()));
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
fn _00_open_write_main_stamps_schema_version() {
  let path = temp_path("version");
  let conn = open_write_main(&path);
  drop(conn);
  assert_eq!(read_user_version(&path), crate::database::SCHEMA_VERSION);
  cleanup(&path);
}

// full pipeline (merge data -> rebuild hierarchy/rtree/tantivy -> optimize). ignored by default
// because it builds a tantivy index on disk; run with `cargo test -- --ignored end_to_end`.
#[test]
#[ignore]
fn _01_end_to_end_merge_into_a_fresh_base_rebuilds_indexes() {
  let base_path = temp_path("e2e_base");
  let source_a = temp_path("e2e_source_a");
  let source_b = temp_path("e2e_source_b");
  let index_dir = format!("{}.tantivy", temp_path("e2e_index"));

  let way1 = pack_admin_id(admin_id_kind::way, 1) as i64;
  build_source(&source_a, &[make_way(1)], &[make_house(100, way1, "10")]);
  let way3 = pack_admin_id(admin_id_kind::way, 3) as i64;
  build_source(&source_b, &[make_way(3)], &[make_house(200, way3, "20")]);

  // base does not exist yet — merge must create it fresh and populate it from both sources.
  assert!(!std::path::Path::new(&base_path).exists());
  let preset = crate::presets::resolve(None);
  super::command_handler_merge(
    &base_path,
    &[source_a.clone(), source_b.clone()],
    &index_dir,
    &preset,
  );

  assert!(std::path::Path::new(&base_path).exists(), "base was created");
  let conn = crate::database::open_readonly(&base_path);
  assert_eq!(count(&conn, "SELECT COUNT(*) FROM admin_levels"), 2);
  assert_eq!(count(&conn, "SELECT COUNT(*) FROM house_numbers"), 2);
  // derived artifacts were rebuilt over the unified set.
  assert!(count(&conn, "SELECT COUNT(*) FROM admin_levels_rtree") >= 2);
  assert!(count(&conn, "SELECT COUNT(*) FROM admin_levels_hierarchy") >= 2);
  drop(conn);

  cleanup(&base_path);
  cleanup(&source_a);
  cleanup(&source_b);
  let _ = std::fs::remove_dir_all(&index_dir);
  let _ = std::fs::remove_file(crate::database::osm_data_path(&base_path));
}

// a named level-12 street at a specific location. distinct coordinates let the rtree/hierarchy and the
// coordinate queries tell streets apart (make_way puts everything at the same point).
fn make_street_at(name: &str, way_id: u64, lon: f64, lat: f64) -> admin_levels_row {
  admin_levels_row {
    relation_id: None,
    way_id: Some(way_id),
    admin_level: 12,
    wkb: Geometry::LineString(LineString(vec![
      Coord { x: lon, y: lat },
      Coord { x: lon + 0.0005, y: lat },
    ]))
    .into(),
    name: name.to_string(),
    country_iso_code: None,
    post_code: None,
  }
}

// the matched street is the most specific admin_level (highest level) of the top result.
fn top_street_name(out: &query::query_output) -> Option<String> {
  out
    .matches
    .first()
    .and_then(|m| m.admin_levels.iter().max_by_key(|a| a.level))
    .map(|a| a.name.clone())
}

fn cleanup_build(sqlite_path: &str) {
  cleanup(sqlite_path);
  let osm = crate::database::osm_data_path(sqlite_path);
  for suffix in ["", "-wal", "-shm"] {
    let _ = std::fs::remove_file(format!("{osm}{suffix}"));
  }
}

// a build merged from two separately-built regions must answer queries identically to a single build
// of the same regions combined. merge re-derives every index from the unified raw data and admin_level
// ids derive from osm ids (deterministic), so for disjoint regions parity is exact. both the merge and
// the combined build use the same preset (DEFAULT). region A sits near (-46.3, -23.9) and region B near
// (7.4, 43.7) — far apart, disjoint ids — mirroring two disjoint extracts without pbf fixtures.
#[test]
fn _02_merge_matches_single_combined_build_query_parity() {
  let source_a = temp_path("parity_source_a");
  let source_b = temp_path("parity_source_b");
  let merged = temp_path("parity_merged");
  let combined = temp_path("parity_combined");
  let merged_index = format!("{}.tantivy", temp_path("parity_merged_index"));
  let combined_index = format!("{}.tantivy", temp_path("parity_combined_index"));

  let alpha = pack_admin_id(admin_id_kind::way, 1) as i64;
  let gamma = pack_admin_id(admin_id_kind::way, 3) as i64;

  // two regions built separately, then merged into a fresh base (re-derives all indexes). the rows
  // are re-created per db because admin_levels is not Clone.
  build_source(
    &source_a,
    &[
      make_street_at("Rua Alpha", 1, -46.30, -23.90),
      make_street_at("Rua Beta", 2, -46.31, -23.91),
    ],
    &[make_house(100, alpha, "10")],
  );
  build_source(
    &source_b,
    &[
      make_street_at("Rue Gamma", 3, 7.40, 43.70),
      make_street_at("Rue Delta", 4, 7.41, 43.71),
    ],
    &[make_house(200, gamma, "20")],
  );
  super::command_handler_merge(
    &merged,
    &[source_a.clone(), source_b.clone()],
    &merged_index,
    &DEFAULT,
  );

  // the same four streets as a single combined build, then index.
  build_source(
    &combined,
    &[
      make_street_at("Rua Alpha", 1, -46.30, -23.90),
      make_street_at("Rua Beta", 2, -46.31, -23.91),
      make_street_at("Rue Gamma", 3, 7.40, 43.70),
      make_street_at("Rue Delta", 4, 7.41, 43.71),
    ],
    &[make_house(100, alpha, "10"), make_house(200, gamma, "20")],
  );
  crate::cli::index::command_handler_index(
    &combined,
    &combined_index,
    None,
    &DEFAULT.index_user_friendly_name,
  );

  let mconn = crate::database::open_readonly(&merged);
  let cconn = crate::database::open_readonly(&combined);
  let mindex = tantivy::load(Path::new(&merged_index), DEFAULT.index_user_friendly_name.boosts)
    .expect("merged tantivy index missing");
  let cindex = tantivy::load(Path::new(&combined_index), DEFAULT.index_user_friendly_name.boosts)
    .expect("combined tantivy index missing");

  // same data in, same row counts out.
  assert_eq!(
    count(&mconn, "SELECT COUNT(*) FROM admin_levels"),
    count(&cconn, "SELECT COUNT(*) FROM admin_levels"),
    "admin_levels count differs between merged and combined"
  );
  assert_eq!(
    count(&mconn, "SELECT COUNT(*) FROM house_numbers"),
    count(&cconn, "SELECT COUNT(*) FROM house_numbers"),
    "house_numbers count differs between merged and combined"
  );

  // text queries in each region resolve to the same street in both builds.
  for q in ["Alpha", "Beta", "Gamma", "Delta"] {
    let m = top_street_name(&query::run(&mconn, Some(&mindex), q, None, None, None, None, false));
    let c = top_street_name(&query::run(&cconn, Some(&cindex), q, None, None, None, None, false));
    assert!(m.is_some(), "query '{q}' returned no match in the merged build");
    assert_eq!(m, c, "merged vs combined differ for text query '{q}'");
  }

  // reverse geocoding (coordinates) is identical too: a point in each region.
  for coord in ["-23.90,-46.30", "43.70,7.40"] {
    let m = top_street_name(&query::run(&mconn, Some(&mindex), coord, None, None, None, None, false));
    let c = top_street_name(&query::run(&cconn, Some(&cindex), coord, None, None, None, None, false));
    assert_eq!(m, c, "merged vs combined differ for coordinate query '{coord}'");
  }

  cleanup_build(&source_a);
  cleanup_build(&source_b);
  cleanup_build(&merged);
  cleanup_build(&combined);
  let _ = std::fs::remove_dir_all(&merged_index);
  let _ = std::fs::remove_dir_all(&combined_index);
}
