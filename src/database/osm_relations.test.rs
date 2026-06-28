use super::*;
use rusqlite::Connection;

fn setup_db() -> Connection {
  let conn = crate::database::open_write(":memory:");
  conn
    .execute_batch("PRAGMA foreign_keys = OFF;")
    .expect("failed to disable fk");
  conn
}

fn make_row(id: u64, admin_level: &str, name: Option<&str>) -> osm_relation_row {
  use crate::extract::osm_data::osm_relations::osm_relation;
  let mut tags = std::collections::HashMap::new();
  tags.insert("admin_level".to_string(), admin_level.to_string());
  if let Some(n) = name {
    tags.insert("name".to_string(), n.to_string());
  }
  let relation = osm_relation {
    id: id as i64,
    tags,
    members: vec![],
  };
  let mut payload = Vec::new();
  crate::extract::osm_data::jsonb_encode::encoder::new()
    .encode_osm_relation(&mut payload, &relation);
  osm_relation_row {
    id,
    osm_pbf_chunk_id: 0,
    payload,
  }
}

#[test]
fn _00_insert_rows_persists_all_rows() {
  let conn = setup_db();
  insert_rows(
    &conn,
    &[
      make_row(1, "4", Some("Brazil")),
      make_row(2, "8", Some("São Paulo")),
    ],
  );
  // ensure total
  let count: i64 = conn
    .query_row("SELECT COUNT(*) FROM osm_data.osm_relations", [], |row| row.get(0))
    .expect("failed to count");
  assert_eq!(count, 2);
  // ensure json tag admin_level is saved as expected
  let id: i64 = conn
    .query_row(
      "SELECT id FROM osm_data.osm_relations WHERE JSON_EXTRACT(payload, '$.tags.admin_level') = '4'",
      [],
      |row| row.get(0),
    )
    .expect("failed to query");
  assert_eq!(id, 1);
  // ensure json tag name is saved as expected
  let id: i64 = conn
    .query_row(
      "SELECT id FROM osm_data.osm_relations WHERE JSON_EXTRACT(payload, '$.tags.name') = 'São Paulo'",
      [],
      |row| row.get(0),
    )
    .expect("failed to query");
  assert_eq!(id, 2);
}

#[test]
fn _01_insert_rows_ignores_duplicate_ids() {
  let conn = setup_db();
  insert_rows(&conn, &[make_row(1, "4", Some("Brazil"))]);
  let mut level: String = conn
    .query_row(
      "SELECT JSON_EXTRACT(payload, '$.tags.admin_level') FROM osm_data.osm_relations WHERE id = 1",
      [],
      |row| row.get(0),
    )
    .expect("failed to query");
  assert_eq!(level, "4");
  insert_rows(&conn, &[make_row(1, "8", Some("São Paulo"))]);
  let count: i64 = conn
    .query_row("SELECT COUNT(*) FROM osm_data.osm_relations", [], |row| row.get(0))
    .expect("failed to count");
  assert_eq!(count, 1);
  level = conn
    .query_row(
      "SELECT JSON_EXTRACT(payload, '$.tags.admin_level') FROM osm_data.osm_relations WHERE id = 1",
      [],
      |row| row.get(0),
    )
    .expect("failed to query");
  assert_eq!(level, "4");
}

#[test]
fn _02_all_ids_by_admin_level_returns_only_matching_level() {
  let conn = setup_db();
  insert_rows(
    &conn,
    &[
      make_row(1, "4", Some("Brazil")),
      make_row(2, "4", Some("Argentina")),
      make_row(3, "8", Some("São Paulo")),
      make_row(4, "9", None),
    ],
  );
  let mut ids = all_ids_by_admin_level(&conn, 4);
  ids.sort();
  assert_eq!(ids, vec![1, 2]);
  ids = all_ids_by_admin_level(&conn, 8);
  assert_eq!(ids, vec![3]);
  ids = all_ids_by_admin_level(&conn, 1);
  assert_eq!(ids.len(), 0);
  // sem nome deve ser ignorado
  ids = all_ids_by_admin_level(&conn, 9);
  assert_eq!(ids.len(), 0);
}

#[test]
fn _04_remaining_ids_excludes_relations_already_present_in_admin_levels() {
  let conn = setup_db();
  insert_rows(
    &conn,
    &[
      make_row(1, "4", Some("A")),
      make_row(2, "4", Some("B")),
      make_row(3, "4", Some("C")),
      make_row(4, "5", Some("D")),
      make_row(5, "6", None),
    ],
  );
  conn
    .execute(
      "
      INSERT INTO admin_levels (relation_id, admin_level, wkb, name)
      VALUES
      (1, 4, zeroblob(1), 'A'),
      (4, 5, zeroblob(1), 'D')
    ",
      rusqlite::params![],
    )
    .expect("failed to insert admin_level");
  let mut ids = remaining_ids_by_admin_level(&conn, 4);
  ids.sort();
  assert_eq!(ids, vec![2, 3]);
  ids = remaining_ids_by_admin_level(&conn, 5);
  assert_eq!(ids.len(), 0);
  ids = remaining_ids_by_admin_level(&conn, 6);
  assert_eq!(ids.len(), 0);
}

// fn make_node_row(id: i64, lat: f64, lon: f64) -> crate::database::osm_nodes::osm_node_row {
//   // todo!()
// }

// fn make_way_row(id: i64, refs: Vec<i64>) -> crate::database::osm_ways::osm_way_row {
//   // todo!()
// }

// fn make_relation_row(id: i64, admin_level: &str, name: &str, way_ids: Vec<i64>) -> osm_relations {
//   // todo!()
// }

#[test]
fn _05_returns_coords_for_single_relation() {
  // todo!()
}

#[test]
fn _06_returns_empty_for_unknown_ids() {
  // todo!()
}

#[test]
fn _07_preserves_node_sequence_order_within_way() {
  // todo!()
}

#[test]
fn _08_ignores_non_way_members() {
  // todo!()
}

#[test]
fn _09_returns_coords_for_multiple_relations_in_chunk() {
  // todo!()
}
