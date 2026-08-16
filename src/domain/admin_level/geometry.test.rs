use super::*;
use geo::{Coord, LineString};

#[test]
fn _00_mbr_center_reads_the_spatialite_bbox_center() {
  use rusqlite::types::{ToSqlOutput, Value};
  let geom: admin_geometry = Geometry::LineString(LineString(vec![
    Coord { x: 10.0, y: 20.0 },
    Coord { x: 30.0, y: 40.0 },
  ]))
  .into();
  let blob: Vec<u8> = match geom.to_sql().expect("to_sql") {
    ToSqlOutput::Owned(Value::Blob(b)) => b,
    ToSqlOutput::Borrowed(ValueRef::Blob(b)) => b.to_vec(),
    _ => panic!("expected a blob"),
  };
  let (cx, cy) = mbr_center(&blob).expect("mbr center");
  assert!((cx - 20.0).abs() < 1e-9, "cx={cx}");
  assert!((cy - 30.0).abs() < 1e-9, "cy={cy}");
}

#[test]
fn _01_mbr_center_rejects_short_or_invalid_blobs() {
  assert_eq!(mbr_center(&[]), None);
  assert_eq!(mbr_center(&[0u8; 10]), None);
  assert_eq!(mbr_center(&[0xFFu8; 40]), None);
}

#[test]
fn _02_geometry_round_trips_through_the_column_format() {
  use rusqlite::types::{ToSqlOutput, Value};
  let original: admin_geometry = Geometry::LineString(LineString(vec![
    Coord { x: -46.3198, y: -23.9724 },
    Coord { x: -46.3197, y: -23.9723 },
  ]))
  .into();
  let blob: Vec<u8> = match original.to_sql().expect("to_sql") {
    ToSqlOutput::Owned(Value::Blob(b)) => b,
    ToSqlOutput::Borrowed(ValueRef::Blob(b)) => b.to_vec(),
    _ => panic!("expected a blob"),
  };

  let read = admin_geometry::column_result(ValueRef::Blob(&blob)).expect("column_result");
  match read.geometry() {
    Geometry::LineString(ls) => {
      assert_eq!(ls.0.len(), 2);
      assert!((ls.0[0].x - -46.3198).abs() < 1e-9);
      assert!((ls.0[1].y - -23.9723).abs() < 1e-9);
    }
    other => panic!("expected a LineString, got {other:?}"),
  }
}

#[test]
fn _03_unreadable_blob_degrades_to_an_empty_geometry() {
  let read = admin_geometry::column_result(ValueRef::Blob(&[0u8; 8])).expect("column_result");
  assert!(matches!(
    read.geometry(),
    Geometry::GeometryCollection(collection) if collection.0.is_empty()
  ));
}
