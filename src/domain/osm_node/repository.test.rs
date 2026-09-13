use crate::domain::pbf_fixtures::{insert_node, memory_db};

fn count(conn: &rusqlite::Connection) -> i64 {
  conn
    .query_row("SELECT COUNT(*) FROM osm_data.osm_nodes", [], |row| row.get(0))
    .expect("failed to count osm_nodes")
}

#[test]
fn _00_keeps_the_first_row_of_an_id_and_reads_the_payload_back() {
  let conn = memory_db();
  insert_node(&conn, 1, -46.6333, -23.5505, &[("addr:housenumber", "10")]);
  insert_node(&conn, 2, 2.3522, 48.8566, &[]);
  insert_node(&conn, 1, 99.0, 99.0, &[]);

  assert_eq!(count(&conn), 2);
  let (lat, number): (f64, String) = conn
    .query_row(
      "SELECT JSON_EXTRACT(payload, '$.lat'), payload->>'tags'->>'addr:housenumber' FROM osm_data.osm_nodes WHERE id = 1",
      [],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .expect("failed to read the node back");
  assert!((lat - -23.5505).abs() < 1e-9);
  assert_eq!(number, "10");
}
