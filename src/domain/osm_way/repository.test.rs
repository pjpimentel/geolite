use super::{remaining_ids_by_tags, way_coords_chunk};
use crate::domain::admin_level::level;
use crate::domain::osm_way::way_filter;
use crate::domain::pbf_fixtures::{insert_node, insert_way, memory_db};

fn count(conn: &rusqlite::Connection) -> i64 {
  conn
    .query_row("SELECT COUNT(*) FROM osm_data.osm_ways", [], |row| row.get(0))
    .expect("failed to count osm_ways")
}

fn sorted(mut ids: Vec<u64>) -> Vec<u64> {
  ids.sort_unstable();
  ids
}

#[test]
fn _00_keeps_the_first_row_of_an_id_and_reads_the_payload_back() {
  let conn = memory_db();
  insert_way(&conn, 1, &[10, 20, 30], &[("highway", "residential"), ("name", "Main St")]);
  insert_way(&conn, 2, &[40, 50], &[]);
  insert_way(&conn, 1, &[10, 20], &[("name", "New St")]);

  assert_eq!(count(&conn), 2);
  let (name, refs_len): (String, i64) = conn
    .query_row(
      "SELECT JSON_EXTRACT(payload, '$.tags.name'), JSON_ARRAY_LENGTH(JSON_EXTRACT(payload, '$.refs')) FROM osm_data.osm_ways WHERE id = 1",
      [],
      |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .expect("failed to read the way back");
  assert_eq!(name, "Main St");
  assert_eq!(refs_len, 3);
}

#[test]
fn _01_remaining_ids_by_tags_skips_ways_indexed_at_the_level_and_nameless_ways() {
  let conn = memory_db();
  insert_way(&conn, 1, &[], &[("name", "A"), ("highway", "residential")]);
  insert_way(&conn, 2, &[], &[("name", "B"), ("highway", "residential")]);
  insert_way(&conn, 3, &[], &[("name", "C"), ("highway", "residential")]);
  insert_way(&conn, 4, &[], &[("highway", "residential")]);
  conn
    .execute(
      "INSERT INTO admin_levels (way_id, admin_level, wkb, name) VALUES (1, 12, ZEROBLOB(1), 'A')",
      [],
    )
    .expect("failed to insert the indexed way");

  let ids = remaining_ids_by_tags(
    &conn,
    level::street,
    &[way_filter::include_highway_residential],
  );

  assert_eq!(sorted(ids), vec![2, 3]);
}

#[test]
fn _02_every_filter_selects_exactly_the_ways_it_names() {
  let conn = memory_db();
  let tagged = [
    ("place", "neighbourhood"),
    ("place", "suburb"),
    ("highway", "residential"),
    ("highway", "primary"),
    ("highway", "secondary"),
    ("highway", "tertiary"),
    ("highway", "unclassified"),
    ("highway", "living_street"),
    ("leisure", "park"),
    ("building", "yes"),
    ("waterway", "river"),
  ];
  for (i, &(key, value)) in tagged.iter().enumerate() {
    let id = i as u64 + 1;
    let name = format!("W{id}");
    insert_way(&conn, id, &[], &[("name", name.as_str()), (key, value)]);
  }
  let selected = |filter: way_filter| sorted(remaining_ids_by_tags(&conn, level::street, &[filter]));

  assert_eq!(selected(way_filter::include_place_neighbourhood), vec![1]);
  assert_eq!(selected(way_filter::include_place_suburb), vec![2]);
  assert_eq!(selected(way_filter::include_highway_residential), vec![3]);
  assert_eq!(selected(way_filter::include_highway_primary), vec![4]);
  assert_eq!(selected(way_filter::include_highway_secondary), vec![5]);
  assert_eq!(selected(way_filter::include_highway_tertiary), vec![6]);
  assert_eq!(selected(way_filter::include_highway_unclassified), vec![7]);
  assert_eq!(selected(way_filter::include_highway_living_street), vec![8]);
  assert_eq!(
    selected(way_filter::exclude_place_neighbourhood),
    vec![2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
  );
  assert_eq!(
    selected(way_filter::exclude_place_suburb),
    vec![1, 3, 4, 5, 6, 7, 8, 9, 10, 11]
  );
  assert_eq!(
    selected(way_filter::exclude_leisure_park),
    vec![1, 2, 3, 4, 5, 6, 7, 8, 10, 11]
  );
  assert_eq!(
    selected(way_filter::exclude_building),
    vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 11]
  );
  assert_eq!(
    selected(way_filter::exclude_waterway),
    vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
  );
}

#[test]
fn _03_way_coords_chunk_follows_the_ref_order_and_normalises_the_post_code() {
  let conn = memory_db();
  insert_node(&conn, 1, -46.6, -23.5, &[]);
  insert_node(&conn, 2, -46.61, -23.51, &[]);
  insert_node(&conn, 3, -46.62, -23.52, &[]);
  insert_way(
    &conn,
    7,
    &[3, 1, 2],
    &[
      ("name", "Rua Alfa"),
      ("name:pt", "Rua Beta"),
      ("addr:postcode", " 11010-100 "),
    ],
  );
  insert_way(&conn, 8, &[2], &[("name", "Rua Gama")]);

  let rows = way_coords_chunk(&conn, &[8, 7], &["name:pt", "name"]);

  let order: Vec<(u64, f64, f64)> = rows.iter().map(|r| (r.way_id, r.lon, r.lat)).collect();
  assert_eq!(
    order,
    vec![
      (7, -46.62, -23.52),
      (7, -46.6, -23.5),
      (7, -46.61, -23.51),
      (8, -46.61, -23.51)
    ]
  );
  assert_eq!(rows[0].way_name, "Rua Beta");
  assert_eq!(rows[0].post_code.as_deref(), Some("11010-100"));
  assert_eq!(rows[3].way_name, "Rua Gama");
  assert_eq!(rows[3].post_code, None);
}

#[test]
fn _04_way_coords_chunk_answers_nothing_for_unknown_ids() {
  let conn = memory_db();
  insert_node(&conn, 1, -46.6, -23.5, &[]);
  insert_way(&conn, 7, &[1], &[("name", "Rua Alfa")]);

  assert!(way_coords_chunk(&conn, &[99], &["name"]).is_empty());
}
