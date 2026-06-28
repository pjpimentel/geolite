use super::*;

fn setup_db() -> Connection {
  let conn = crate::database::open_write(":memory:");
  conn
    .execute_batch("PRAGMA foreign_keys = OFF;")
    .expect("failed to disable fk");
  conn
}

fn make_row(id: u64, refs: Vec<i64>, tags: Vec<(&str, &str)>) -> osm_way_row {
  let way = crate::extract::osm_data::osm_ways::osm_way {
    id: id as i64,
    refs,
    tags: tags
      .into_iter()
      .map(|(k, v)| (k.to_string(), v.to_string()))
      .collect(),
  };
  let mut payload = Vec::new();
  crate::extract::osm_data::jsonb_encode::encoder::new().encode_osm_way(&mut payload, &way);
  osm_way_row {
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
      make_row(
        1,
        vec![10, 20, 30],
        vec![("highway", "residential"), ("name", "Main St")],
      ),
      make_row(2, vec![40, 50], vec![]),
    ],
  );
  let count: i64 = conn
    .query_row("SELECT COUNT(*) FROM osm_data.osm_ways", [], |row| row.get(0))
    .expect("failed to count");
  assert_eq!(count, 2);
  let name: String = conn
    .query_row(
      "SELECT json_extract(payload, '$.tags.name') FROM osm_data.osm_ways WHERE id = 1",
      [],
      |row| row.get(0),
    )
    .expect("failed to query name");
  assert_eq!(name, "Main St");
  let refs_len: i64 = conn
    .query_row(
      "SELECT json_array_length(json_extract(payload, '$.refs')) FROM osm_data.osm_ways WHERE id = 1",
      [],
      |row| row.get(0),
    )
    .expect("failed to count refs");
  assert_eq!(refs_len, 3);
}

#[test]
fn _01_insert_rows_ignores_duplicate_ids() {
  let conn = setup_db();
  insert_rows(&conn, &[make_row(1, vec![10], vec![("name", "Old St")])]);
  let name: String = conn
    .query_row(
      "SELECT json_extract(payload, '$.tags.name') FROM osm_data.osm_ways WHERE id = 1",
      [],
      |row| row.get(0),
    )
    .expect("failed to query name");
  assert_eq!(name, "Old St");
  insert_rows(
    &conn,
    &[make_row(1, vec![10, 20], vec![("name", "New St")])],
  );
  let name: String = conn
    .query_row(
      "SELECT json_extract(payload, '$.tags.name') FROM osm_data.osm_ways WHERE id = 1",
      [],
      |row| row.get(0),
    )
    .expect("failed to query name");
  assert_eq!(name, "Old St");
  let count: i64 = conn
    .query_row("SELECT COUNT(*) FROM osm_data.osm_ways", [], |row| row.get(0))
    .expect("failed to count");
  assert_eq!(count, 1);
}

#[test]
fn _02_remaining_ids_by_tags_excludes_ways_already_in_admin_levels() {
  let conn = setup_db();
  insert_rows(
    &conn,
    &[
      make_row(1, vec![], vec![("name", "A"), ("highway", "residential")]),
      make_row(2, vec![], vec![("name", "B"), ("highway", "residential")]),
      make_row(3, vec![], vec![("name", "C"), ("highway", "residential")]),
      make_row(4, vec![], vec![("highway", "residential")]),
    ],
  );
  conn.execute(
    "INSERT INTO admin_levels (way_id, admin_level, wkb, name) VALUES (1, 12, zeroblob(1), 'A')",
    rusqlite::params![],
  ).expect("failed to insert admin_level");
  let mut ids = remaining_ids_by_tags(&conn, 12, &[filters::include_highway_residential]);
  ids.sort();
  assert_eq!(ids, vec![2, 3]);
}

#[test]
fn _03_all_mapped_filters_works_as_expected() {
  let conn = setup_db();
  insert_rows(
    &conn,
    &[
      make_row(1, vec![], vec![("name", "W1"), ("place", "neighbourhood")]),
      make_row(2, vec![], vec![("name", "W2"), ("place", "suburb")]),
      make_row(3, vec![], vec![("name", "W3"), ("highway", "residential")]),
      make_row(4, vec![], vec![("name", "W4"), ("highway", "primary")]),
      make_row(5, vec![], vec![("name", "W5"), ("highway", "secondary")]),
      make_row(6, vec![], vec![("name", "W6"), ("highway", "tertiary")]),
      make_row(7, vec![], vec![("name", "W7"), ("highway", "unclassified")]),
      make_row(
        8,
        vec![],
        vec![("name", "W8"), ("highway", "living_street")],
      ),
      make_row(9, vec![], vec![("name", "W9"), ("leisure", "park")]),
      make_row(10, vec![], vec![("name", "W10"), ("building", "yes")]),
      make_row(11, vec![], vec![("name", "W11"), ("waterway", "river")]),
    ],
  );

  assert_eq!(
    remaining_ids_by_tags(&conn, 99, &[filters::include_place_neighbourhood]),
    vec![1]
  );
  assert_eq!(
    remaining_ids_by_tags(&conn, 99, &[filters::include_place_suburb]),
    vec![2]
  );
  assert_eq!(
    remaining_ids_by_tags(&conn, 99, &[filters::include_highway_residential]),
    vec![3]
  );
  assert_eq!(
    remaining_ids_by_tags(&conn, 99, &[filters::include_highway_primary]),
    vec![4]
  );
  assert_eq!(
    remaining_ids_by_tags(&conn, 99, &[filters::include_highway_secondary]),
    vec![5]
  );
  assert_eq!(
    remaining_ids_by_tags(&conn, 99, &[filters::include_highway_tertiary]),
    vec![6]
  );
  assert_eq!(
    remaining_ids_by_tags(&conn, 99, &[filters::include_highway_unclassified]),
    vec![7]
  );
  assert_eq!(
    remaining_ids_by_tags(&conn, 99, &[filters::include_highway_living_street]),
    vec![8]
  );

  let mut ids = remaining_ids_by_tags(&conn, 99, &[filters::exclude_place_neighbourhood]);
  ids.sort();
  assert_eq!(ids, vec![2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);

  let mut ids = remaining_ids_by_tags(&conn, 99, &[filters::exclude_place_suburb]);
  ids.sort();
  assert_eq!(ids, vec![1, 3, 4, 5, 6, 7, 8, 9, 10, 11]);

  let mut ids = remaining_ids_by_tags(&conn, 99, &[filters::exclude_leisure_park]);
  ids.sort();
  assert_eq!(ids, vec![1, 2, 3, 4, 5, 6, 7, 8, 10, 11]);

  let mut ids = remaining_ids_by_tags(&conn, 99, &[filters::exclude_building]);
  ids.sort();
  assert_eq!(ids, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 11]);

  let mut ids = remaining_ids_by_tags(&conn, 99, &[filters::exclude_waterway]);
  ids.sort();
  assert_eq!(ids, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
}

// TODO: fazer testes para way_coords_chunk
