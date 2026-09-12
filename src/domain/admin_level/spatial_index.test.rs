use std::cell::RefCell;

use geo::{Coord, Geometry, LineString};
use rusqlite::Connection;

use super::super::entity::admin_level;
use super::super::repository::batch_upsert;
use super::super::scale::level;
use super::{recreate, run};

fn square(way_id: u64, origin: f64) -> admin_level {
  let far = origin + 1.0;
  let ring = LineString(vec![
    Coord { x: origin, y: origin },
    Coord { x: far, y: origin },
    Coord { x: far, y: far },
    Coord { x: origin, y: far },
    Coord { x: origin, y: origin },
  ]);
  admin_level {
    relation_id: None,
    way_id: Some(way_id),
    level: level::street,
    wkb: Geometry::LineString(ring).into(),
    name: format!("way_{way_id}"),
    country_iso_code: None,
    post_code: None,
  }
}

fn seeded(path: &str, count: u64) -> Connection {
  let conn = crate::database::open_write(path);
  let rows: Vec<admin_level> = (1..=count).map(|i| square(i, i as f64)).collect();
  batch_upsert(&conn, &rows);
  conn
}

fn file_database(name: &str) -> String {
  let dir = std::env::temp_dir().join(format!("spatial_index_{name}"));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).unwrap();
  dir.join("db.sqlite3").to_str().unwrap().to_string()
}

fn boxes(conn: &Connection) -> Vec<(i64, f64, f64, f64, f64)> {
  conn
    .prepare("SELECT id, min_lon, max_lon, min_lat, max_lat FROM admin_levels_rtree ORDER BY id")
    .expect("failed to prepare")
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
    .expect("failed to query the rtree")
    .map(|r| r.expect("failed to read a box"))
    .collect()
}

#[test]
fn _00_run_fills_the_rtree_with_one_box_per_geometry() {
  let conn = seeded(&file_database("t00"), 3);
  let seen = RefCell::new(Vec::new());
  run(&conn, |p| seen.borrow_mut().push((p.total, p.processed)));

  let stored = boxes(&conn);
  assert_eq!(stored.len(), 3, "one box per row with geometry");
  assert_eq!(stored[0], (2, 1.0, 2.0, 1.0, 2.0), "way 1 packs to id 2 and spans 1..2");
  assert_eq!(stored[2], (6, 3.0, 4.0, 3.0, 4.0));
  let seen = seen.into_inner();
  assert_eq!(seen.first(), Some(&(Some(3), 0)), "progress starts at zero with the total");
  assert_eq!(seen.last().map(|s| s.1), Some(3), "progress ends at the total");
}

#[test]
fn _01_an_in_memory_database_takes_the_sequential_path() {
  let conn = seeded(":memory:", 2);
  let seen = RefCell::new(Vec::new());
  run(&conn, |p| seen.borrow_mut().push(p.processed));
  assert_eq!(boxes(&conn).len(), 2);
  assert_eq!(seen.into_inner(), vec![0, 2], "one page, reported once");
}

#[test]
fn _02_recreate_empties_the_rtree() {
  let conn = seeded(":memory:", 2);
  run(&conn, |_| {});
  assert_eq!(boxes(&conn).len(), 2);
  recreate(&conn);
  assert!(boxes(&conn).is_empty());
}
