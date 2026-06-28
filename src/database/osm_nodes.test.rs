use super::*;

fn setup_db() -> Connection {
  let conn = crate::database::open_write(":memory:");
  conn
    .execute_batch("PRAGMA foreign_keys = OFF;")
    .expect("failed to disable fk");
  conn
}

fn make_row(id: u64, lat: f64, lon: f64, tags: Vec<(&str, &str)>) -> osm_node_row {
  let node = crate::extract::osm_data::osm_nodes::osm_node {
    id: id as i64,
    lat,
    lon,
    tags: tags
      .into_iter()
      .map(|(k, v)| (k.to_string(), v.to_string()))
      .collect(),
  };
  let mut payload = Vec::new();
  crate::extract::osm_data::jsonb_encode::encoder::new().encode_osm_node(&mut payload, &node);
  osm_node_row {
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
      make_row(1, -23.5505, -46.6333, vec![("addr:housenumber", "10")]),
      make_row(2, 48.8566, 2.3522, vec![]),
    ],
  );
  let count: i64 = conn
    .query_row("SELECT COUNT(*) FROM osm_data.osm_nodes", [], |row| row.get(0))
    .expect("failed to count");
  assert_eq!(count, 2);
  let lat: f64 = conn
    .query_row(
      "SELECT json_extract(payload, '$.lat') FROM osm_data.osm_nodes WHERE id = 1",
      [],
      |row| row.get(0),
    )
    .expect("failed to query lat");
  assert!((lat - -23.5505).abs() < 1e-9);
  let number: String = conn
    .query_row(
      "SELECT payload->>'tags'->>'addr:housenumber' FROM osm_data.osm_nodes WHERE id = 1",
      [],
      |row| row.get(0),
    )
    .expect("failed to query tag");
  assert_eq!(number, "10");
}

#[test]
fn _01_insert_rows_ignores_duplicate_ids() {
  let conn = setup_db();
  insert_rows(&conn, &[make_row(1, 10.0, 20.0, vec![])]);
  insert_rows(&conn, &[make_row(1, 99.0, 99.0, vec![])]);
  let count: i64 = conn
    .query_row("SELECT COUNT(*) FROM osm_data.osm_nodes", [], |row| row.get(0))
    .expect("failed to count");
  assert_eq!(count, 1);
  let lat: f64 = conn
    .query_row(
      "SELECT json_extract(payload, '$.lat') FROM osm_data.osm_nodes WHERE id = 1",
      [],
      |row| row.get(0),
    )
    .expect("failed to query lat");
  assert!((lat - 10.0).abs() < 1e-9);
}
