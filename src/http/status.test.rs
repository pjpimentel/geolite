use super::{assemble, probe_coordinates};

const SQL_CREATE_RTREE: &str = "
  CREATE VIRTUAL TABLE admin_levels_rtree
  USING rtree(id, min_lon, max_lon, min_lat, max_lat)
";

#[test]
fn is_ok_is_true_only_when_both_services_available() {
  assert!(assemble("db.sqlite3", true, true).is_ok);
  assert!(!assemble("db.sqlite3", true, false).is_ok);
  assert!(!assemble("db.sqlite3", false, true).is_ok);
  assert!(!assemble("db.sqlite3", false, false).is_ok);
}

#[test]
fn output_carries_the_database_entry() {
  let out = assemble("database.sqlite3", true, false);
  assert_eq!(out.databases.len(), 1);
  let db = &out.databases[0];
  assert_eq!(db.file, "database.sqlite3");
  assert!(db.text_to_address);
  assert!(!db.coordinates_to_address);
}

#[test]
fn serialized_shape_matches_contract() {
  let out = assemble("database.sqlite3", true, true);
  let v = serde_json::to_value(&out).unwrap();
  assert_eq!(v["is_ok"], serde_json::json!(true));
  assert_eq!(v["databases"][0]["file"], "database.sqlite3");
  assert_eq!(v["databases"][0]["text_to_address"], serde_json::json!(true));
  assert_eq!(v["databases"][0]["coordinates_to_address"], serde_json::json!(true));
}

#[test]
fn coordinates_probe_true_when_rtree_table_exists() {
  let conn = rusqlite::Connection::open_in_memory().unwrap();
  conn.execute_batch(SQL_CREATE_RTREE).unwrap();
  assert!(probe_coordinates(&conn));
}

#[test]
fn coordinates_probe_false_when_rtree_table_missing() {
  let conn = rusqlite::Connection::open_in_memory().unwrap();
  assert!(!probe_coordinates(&conn));
}
