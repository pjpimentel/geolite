use super::{
  SQL_CANDIDATES_BY_ADMIN_LEVEL, all_ids_by_admin_level, osm_relations, relation_coords_chunk,
  remaining_ids_by_admin_level,
};
use crate::admin_level::level;
use crate::osm_pbf_file::pbf_fixtures::{insert_node, insert_relation, insert_way, memory_db};
use crate::database::table;

fn count(conn: &rusqlite::Connection) -> i64 {
  conn
    .query_row("SELECT COUNT(*) FROM osm_data.osm_relations", [], |row| row.get(0))
    .expect("failed to count osm_relations")
}

fn sorted(mut ids: Vec<u64>) -> Vec<u64> {
  ids.sort_unstable();
  ids
}

fn insert_boundary(conn: &rusqlite::Connection, id: u64, admin_level: &str, name: Option<&str>) {
  let mut tags = vec![("admin_level", admin_level)];
  if let Some(name) = name {
    tags.push(("name", name));
  }
  insert_relation(conn, id, &[], &tags);
}

#[test]
fn _00_keeps_the_first_row_of_an_id_and_reads_the_payload_back() {
  let conn = memory_db();
  insert_boundary(&conn, 1, "4", Some("Brazil"));
  insert_boundary(&conn, 2, "8", Some("São Paulo"));
  insert_boundary(&conn, 1, "8", Some("Argentina"));

  assert_eq!(count(&conn), 2);
  let by_level: u64 = conn
    .query_row(
      "SELECT id FROM osm_data.osm_relations WHERE JSON_EXTRACT(payload, '$.tags.admin_level') = '4'",
      [],
      |row| row.get(0),
    )
    .expect("failed to read by level");
  let by_name: u64 = conn
    .query_row(
      "SELECT id FROM osm_data.osm_relations WHERE JSON_EXTRACT(payload, '$.tags.name') = 'São Paulo'",
      [],
      |row| row.get(0),
    )
    .expect("failed to read by name");
  assert_eq!((by_level, by_name), (1, 2));
}

#[test]
fn _01_all_ids_by_admin_level_answers_the_named_relations_of_the_level() {
  let conn = memory_db();
  insert_boundary(&conn, 1, "4", Some("Brazil"));
  insert_boundary(&conn, 2, "4", Some("Argentina"));
  insert_boundary(&conn, 3, "8", Some("São Paulo"));
  insert_boundary(&conn, 4, "9", None);

  assert_eq!(sorted(all_ids_by_admin_level(&conn, level::state)), vec![1, 2]);
  assert_eq!(all_ids_by_admin_level(&conn, level::city), vec![3]);
  assert!(all_ids_by_admin_level(&conn, level::continent).is_empty());
  assert!(all_ids_by_admin_level(&conn, level::locality).is_empty());
}

#[test]
fn _02_remaining_ids_by_admin_level_skips_relations_indexed_at_the_level() {
  let conn = memory_db();
  insert_boundary(&conn, 1, "4", Some("A"));
  insert_boundary(&conn, 2, "4", Some("B"));
  insert_boundary(&conn, 3, "4", Some("C"));
  insert_boundary(&conn, 4, "5", Some("D"));
  insert_boundary(&conn, 5, "6", None);
  conn
    .execute_batch(
      "INSERT INTO admin_levels (relation_id, admin_level, wkb, name)
       VALUES (1, 4, ZEROBLOB(1), 'A'), (4, 5, ZEROBLOB(1), 'D');",
    )
    .expect("failed to insert the indexed relations");

  assert_eq!(
    sorted(remaining_ids_by_admin_level(&conn, level::state)),
    vec![2, 3]
  );
  assert!(remaining_ids_by_admin_level(&conn, level::district).is_empty());
  assert!(remaining_ids_by_admin_level(&conn, level::county).is_empty());
}

#[test]
fn _03_relation_coords_chunk_reads_a_relation_with_its_name_and_codes() {
  let conn = memory_db();
  insert_node(&conn, 1, -46.3, -23.9, &[]);
  insert_node(&conn, 2, -46.31, -23.91, &[]);
  insert_way(&conn, 10, &[1, 2], &[]);
  insert_relation(
    &conn,
    100,
    &[(1, 10, "outer")],
    &[
      ("name", "Santos"),
      ("name:pt", "Santos-PT"),
      ("ISO3166-1", " br "),
      ("postal_code", "11010-100"),
    ],
  );

  let rows = relation_coords_chunk(&conn, &[100], &["name:pt", "name"]);

  assert_eq!(rows.len(), 2);
  let first = &rows[0];
  assert_eq!(first.relation_id, 100);
  assert_eq!(first.relation_name, "Santos-PT");
  assert_eq!(first.country_iso_code.as_deref(), Some("BR"));
  assert_eq!(first.post_code.as_deref(), Some("11010-100"));
  assert_eq!((first.way_order, first.way_id), (0, 10));
  assert_eq!((first.lon, first.lat), (-46.3, -23.9));
  assert_eq!((rows[1].lon, rows[1].lat), (-46.31, -23.91));
}

#[test]
fn _04_relation_coords_chunk_answers_nothing_for_unknown_ids() {
  let conn = memory_db();
  insert_node(&conn, 1, -46.3, -23.9, &[]);
  insert_way(&conn, 10, &[1], &[]);
  insert_relation(&conn, 100, &[(1, 10, "outer")], &[("name", "Santos")]);

  assert!(relation_coords_chunk(&conn, &[999], &["name"]).is_empty());
}

#[test]
fn _05_relation_coords_chunk_keeps_the_node_sequence_of_each_way() {
  let conn = memory_db();
  insert_node(&conn, 1, -46.3, -23.9, &[]);
  insert_node(&conn, 2, -46.31, -23.91, &[]);
  insert_node(&conn, 3, -46.32, -23.92, &[]);
  insert_way(&conn, 10, &[3, 1, 2], &[]);
  insert_relation(&conn, 100, &[(1, 10, "outer")], &[("name", "Santos")]);

  let lons: Vec<f64> = relation_coords_chunk(&conn, &[100], &["name"])
    .iter()
    .map(|row| row.lon)
    .collect();

  assert_eq!(lons, vec![-46.32, -46.3, -46.31]);
}

#[test]
fn _06_relation_coords_chunk_ignores_members_that_are_not_ways() {
  let conn = memory_db();
  insert_node(&conn, 1, -46.3, -23.9, &[]);
  insert_node(&conn, 2, -46.31, -23.91, &[]);
  insert_way(&conn, 10, &[1, 2], &[]);
  insert_relation(&conn, 200, &[(1, 10, "outer")], &[("name", "Inner")]);
  insert_relation(
    &conn,
    100,
    &[(0, 1, "admin_centre"), (1, 10, "outer"), (2, 200, "subarea")],
    &[("name", "Santos")],
  );

  let rows = relation_coords_chunk(&conn, &[100], &["name"]);

  let members: Vec<(u32, u64)> = rows.iter().map(|row| (row.way_order, row.way_id)).collect();
  assert_eq!(members, vec![(1, 10), (1, 10)]);
}

#[test]
fn _07_relation_coords_chunk_orders_by_relation_then_by_member_order() {
  let conn = memory_db();
  insert_node(&conn, 1, -46.3, -23.9, &[]);
  insert_node(&conn, 2, -46.31, -23.91, &[]);
  insert_node(&conn, 3, -46.32, -23.92, &[]);
  insert_way(&conn, 10, &[1], &[]);
  insert_way(&conn, 11, &[2], &[]);
  insert_way(&conn, 12, &[3], &[]);
  insert_relation(
    &conn,
    100,
    &[(1, 11, "outer"), (1, 10, "outer")],
    &[("name", "Santos")],
  );
  insert_relation(&conn, 101, &[(1, 12, "outer")], &[("name", "Guarujá")]);

  let rows = relation_coords_chunk(&conn, &[101, 100], &["name"]);

  let order: Vec<(u64, u32, u64)> = rows
    .iter()
    .map(|row| (row.relation_id, row.way_order, row.way_id))
    .collect();
  assert_eq!(order, vec![(100, 0, 11), (100, 1, 10), (101, 0, 12)]);
  assert_eq!(rows[2].relation_name, "Guarujá");
}

#[test]
fn _08_the_candidate_query_runs_on_the_admin_level_expression_index() {
  let conn = memory_db();
  osm_relations::create_indexes(&conn);

  let plan: Vec<String> = conn
    .prepare(&format!("EXPLAIN QUERY PLAN {SQL_CANDIDATES_BY_ADMIN_LEVEL}"))
    .expect("failed to explain the candidate query")
    .query_map(["4"], |row| row.get::<_, String>(3))
    .expect("failed to read the plan")
    .collect::<Result<_, _>>()
    .expect("failed to collect the plan");

  assert!(
    plan
      .iter()
      .any(|step| step.contains("USING INDEX osm_relations_search_by_admin_level")),
    "plan: {plan:?}"
  );
}
