use crate::domain::admin_level::level;
use super::batch_upsert;
use crate::domain::admin_level::admin_level as admin_levels_row;
use geo::{Coord, Geometry, LineString};

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

fn make_geometry() -> super::admin_geometry {
  Geometry::LineString(LineString(vec![
    Coord { x: 0.0, y: 0.0 },
    Coord { x: 0.001, y: 0.001 },
  ]))
  .into()
}

fn make_way_row(way_id: u64) -> admin_levels_row {
  admin_levels_row {
    relation_id: None,
    way_id: Some(way_id),
    level: level::street,
    wkb: make_geometry(),
    name: format!("way_{}", way_id),
    country_iso_code: None,
    post_code: None,
  }
}

fn make_relation_row(relation_id: u64) -> admin_levels_row {
  admin_levels_row {
    relation_id: Some(relation_id),
    way_id: None,
    level: level::state,
    wkb: make_geometry(),
    name: format!("rel_{}", relation_id),
    country_iso_code: None,
    post_code: None,
  }
}

fn select_pairs(conn: &super::Connection, sql: &str) -> Vec<(i64, i64)> {
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
  let rows: Vec<admin_levels_row> = (0..=10).map(make_way_row).collect();
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
  let rows: Vec<admin_levels_row> = (0..=10).map(make_relation_row).collect();
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



