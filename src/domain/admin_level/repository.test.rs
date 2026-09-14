use geo::{Coord, Geometry, LineString};
use rusqlite::Connection;

use super::super::entity::admin_level;
use super::super::geometry::admin_geometry;
use super::super::scale::level;
use super::super::id::admin_level_id;
use super::{
  batch_upsert, geometry_by_ids, load_all_below_street, load_all_names, load_by_ids,
  load_full_by_ids, load_metadata_by_ids, streets_with_centroid, wkt_by_ids,
};
use crate::domain::admin_level_hierarchy::fixtures::{area, street};

const SQL_SELECT_WAY_PAIRS: &str = "
  SELECT way_id, id
  FROM admin_levels
  WHERE way_id IS NOT NULL
  ORDER BY way_id
";

const SQL_SELECT_RELATION_PAIRS: &str = "
  SELECT relation_id, id
  FROM admin_levels
  WHERE relation_id IS NOT NULL
  ORDER BY relation_id
";

fn make_geometry() -> admin_geometry {
  Geometry::LineString(LineString(vec![
    Coord { x: 0.0, y: 0.0 },
    Coord { x: 0.001, y: 0.001 },
  ]))
  .into()
}

fn make_way_row(way_id: u64) -> admin_level {
  admin_level {
    relation_id: None,
    way_id: Some(way_id),
    level: level::street,
    wkb: make_geometry(),
    name: format!("way_{}", way_id),
    country_iso_code: None,
    post_code: None,
  }
}

fn make_relation_row(relation_id: u64) -> admin_level {
  admin_level {
    relation_id: Some(relation_id),
    way_id: None,
    level: level::state,
    wkb: make_geometry(),
    name: format!("rel_{}", relation_id),
    country_iso_code: None,
    post_code: None,
  }
}

fn select_pairs(conn: &Connection, sql: &str) -> Vec<(i64, i64)> {
  let mut stmt = conn.prepare(sql).expect("failed to prepare");
  stmt
    .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read row"))
    .collect()
}

#[test]
fn _00_way_ids_0_to_10_produce_bit_packed_ids() {
  let conn = crate::database::open_write(":memory:");
  let rows: Vec<admin_level> = (0..=10).map(make_way_row).collect();
  batch_upsert(&conn, &rows);

  let pairs = select_pairs(&conn, SQL_SELECT_WAY_PAIRS);
  assert_eq!(
    pairs,
    vec![
      (0, 0),
      (1, 2),
      (2, 4),
      (3, 6),
      (4, 8),
      (5, 10),
      (6, 12),
      (7, 14),
      (8, 16),
      (9, 18),
      (10, 20),
    ],
  );
}

#[test]
fn _01_relation_ids_0_to_10_produce_bit_packed_ids() {
  let conn = crate::database::open_write(":memory:");
  let rows: Vec<admin_level> = (0..=10).map(make_relation_row).collect();
  batch_upsert(&conn, &rows);

  let pairs = select_pairs(&conn, SQL_SELECT_RELATION_PAIRS);
  assert_eq!(
    pairs,
    vec![
      (0, 1),
      (1, 3),
      (2, 5),
      (3, 7),
      (4, 9),
      (5, 11),
      (6, 13),
      (7, 15),
      (8, 17),
      (9, 19),
      (10, 21),
    ],
  );
}

#[test]
fn _02_a_row_with_a_level_outside_the_scale_is_skipped_on_read() {
  let conn = crate::database::open_write(":memory:");
  batch_upsert(&conn, &[make_way_row(1)]);
  conn
    .execute("UPDATE admin_levels SET admin_level = 11", [])
    .expect("failed to write the level outside the scale");
  let id = admin_level_id::from_way(1).raw() as i64;

  assert!(load_by_ids(&conn, &[id]).is_empty());
  assert!(load_all_below_street(&conn).is_empty());
  assert!(load_full_by_ids(&conn, &[id]).is_empty());
  assert!(load_metadata_by_ids(&conn, &[id]).is_empty());
}

#[test]
fn _03_streets_with_centroid_reads_the_mbr_centre_of_each_street() {
  let conn = crate::database::open_write(":memory:");
  batch_upsert(
    &conn,
    &[
      street(2, "street_b", 10.0),
      street(1, "street_a", 0.0),
      area(7, level::city, "city", 0.0, 1.0),
    ],
  );

  let rows = streets_with_centroid(&conn);

  let seen: Vec<(&str, f64, f64)> = rows.iter().map(|r| (r.name.as_str(), r.cx, r.cy)).collect();
  assert_eq!(rows.len(), 2, "only the streets, in id order");
  assert_eq!(seen[0].0, "street_a");
  assert!((seen[0].1 - 0.05).abs() < 1e-9 && seen[0].2.abs() < 1e-9);
  assert_eq!(seen[1].0, "street_b");
  assert!((seen[1].1 - 10.05).abs() < 1e-9 && (seen[1].2 - 10.0).abs() < 1e-9);
}

#[test]
fn _04_geometry_and_wkt_by_ids_answer_only_the_ids_they_know() {
  let conn = crate::database::open_write(":memory:");
  batch_upsert(&conn, &[street(1, "street_a", 0.0), street(2, "street_b", 1.0)]);
  let a = admin_level_id::from_way(1).raw() as i64;

  let geometries = geometry_by_ids(&conn, &[a, 99]);
  let wkt = wkt_by_ids(&conn, &[a, 99]);

  assert_eq!(geometries.len(), 1);
  assert_eq!(geometries[0].0, a);
  assert_eq!(wkt.len(), 1);
  assert!(
    wkt[&a].starts_with("LINESTRING(") && wkt[&a].contains("0.1"),
    "unexpected wkt: {}",
    wkt[&a]
  );
  assert!(geometry_by_ids(&conn, &[]).is_empty());
}

#[test]
fn _05_load_all_names_types_the_level_and_keeps_the_post_code() {
  let conn = crate::database::open_write(":memory:");
  let mut with_post_code = street(1, "street_a", 0.0);
  with_post_code.post_code = Some("01310-100".to_string());
  batch_upsert(&conn, &[with_post_code, area(7, level::city, "city", 0.0, 1.0)]);

  let mut rows = load_all_names(&conn);
  rows.sort_by_key(|r| r.id);

  let seen: Vec<(String, level, Option<String>)> = rows
    .into_iter()
    .map(|r| (r.name, r.admin_level, r.post_code))
    .collect();
  assert_eq!(
    seen,
    vec![
      ("street_a".to_string(), level::street, Some("01310-100".to_string())),
      ("city".to_string(), level::city, None)
    ]
  );
}
