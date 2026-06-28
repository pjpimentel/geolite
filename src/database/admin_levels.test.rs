use super::{
  admin_id_kind, admin_levels as admin_levels_row, batch_upsert, pack_admin_id, unpack_admin_id,
};
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
    admin_level: 12,
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
    admin_level: 4,
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

#[test]
fn _02_pack_admin_id_way_examples() {
  let actual: Vec<u64> = (0u64..=10).map(|n| pack_admin_id(admin_id_kind::way, n)).collect();
  assert_eq!(actual, vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20]);
}

#[test]
fn _03_pack_admin_id_relation_examples() {
  let actual: Vec<u64> = (0u64..=10)
    .map(|n| pack_admin_id(admin_id_kind::relation, n))
    .collect();
  assert_eq!(actual, vec![1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21]);
}

#[test]
fn _05_mbr_center_reads_the_spatialite_bbox_center() {
  use rusqlite::types::{ToSql, ToSqlOutput, Value, ValueRef};
  // bbox is min(10,20) -> max(30,40); the MBR center is (20, 30).
  let geom: super::admin_geometry = Geometry::LineString(LineString(vec![
    Coord { x: 10.0, y: 20.0 },
    Coord { x: 30.0, y: 40.0 },
  ]))
  .into();
  let blob: Vec<u8> = match geom.to_sql().expect("to_sql") {
    ToSqlOutput::Owned(Value::Blob(b)) => b,
    ToSqlOutput::Borrowed(ValueRef::Blob(b)) => b.to_vec(),
    _ => panic!("expected a blob"),
  };
  let (cx, cy) = super::mbr_center(&blob).expect("mbr center");
  assert!((cx - 20.0).abs() < 1e-9, "cx={cx}");
  assert!((cy - 30.0).abs() < 1e-9, "cy={cy}");
}

#[test]
fn _06_mbr_center_rejects_short_or_invalid_blobs() {
  assert_eq!(super::mbr_center(&[]), None);
  assert_eq!(super::mbr_center(&[0u8; 10]), None);
  // valid length but wrong start marker.
  assert_eq!(super::mbr_center(&[0xFFu8; 40]), None);
}

#[test]
fn _04_unpack_admin_id_examples() {
  let actual: Vec<(admin_id_kind, u64)> = (0u64..=11).map(unpack_admin_id).collect();
  assert_eq!(
    actual,
    vec![
      (admin_id_kind::way, 0),
      (admin_id_kind::relation, 0),
      (admin_id_kind::way, 1),
      (admin_id_kind::relation, 1),
      (admin_id_kind::way, 2),
      (admin_id_kind::relation, 2),
      (admin_id_kind::way, 3),
      (admin_id_kind::relation, 3),
      (admin_id_kind::way, 4),
      (admin_id_kind::relation, 4),
      (admin_id_kind::way, 5),
      (admin_id_kind::relation, 5),
    ],
  );
}
