use std::cell::RefCell;

use geo::{Geometry, MultiPolygon, Polygon};
use rusqlite::Connection;

use super::super::fixtures::{area, relation, ring, street, way};
use super::super::repository::{count, load_by_ids};
use super::{extract_polygons, point_in_polygons, point_in_ring, polygon_entry, run};
use crate::domain::admin_level::repository::batch_upsert;
use crate::domain::admin_level::{admin_level, level};
use crate::domain::pbf_fixtures::tempdir_guard;

const COUNTRY: u64 = 1;
const CITY: u64 = 2;
const NEIGHBORHOOD: u64 = 3;
const STREET: u64 = 10;

// a country holding a city holding a neighborhood, and a street inside all three
fn nested_areas() -> Vec<admin_level> {
  vec![
    area(COUNTRY, level::country, "Pais", 0.0, 100.0),
    area(CITY, level::city, "Cidade", 10.0, 20.0),
    area(NEIGHBORHOOD, level::neighborhood, "Bairro", 12.0, 14.0),
    street(STREET, "Rua X", 13.0),
  ]
}

fn chain_of(conn: &Connection, id: i64) -> (Vec<i64>, String) {
  let rows = load_by_ids(conn, &[id]);
  let row = rows.get(&id).expect("a hierarchy row for the id");
  (row.ancestor_ids.clone(), row.user_friendly_name.clone())
}

fn resolved(rows: &[admin_level]) -> Connection {
  let conn = crate::database::open_write(":memory:");
  batch_upsert(&conn, rows);
  run(&conn, |_| {});
  conn
}

#[test]
fn _00_point_in_ring_tells_inside_from_outside() {
  let square = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]];
  assert!(point_in_ring(0.5, 0.5, &square));
  assert!(!point_in_ring(2.0, 2.0, &square));
  assert!(!point_in_ring(0.5, 0.5, &square[..2]), "two points are not a ring");
}

#[test]
fn _01_a_hole_excludes_its_interior() {
  let poly = polygon_entry {
    exterior: vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [0.0, 0.0]],
    interiors: vec![vec![[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0], [4.0, 4.0]]],
  };
  assert!(point_in_polygons(1.0, 1.0, std::slice::from_ref(&poly)));
  assert!(!point_in_polygons(5.0, 5.0, std::slice::from_ref(&poly)), "inside the hole");
  assert!(!point_in_polygons(1.0, 1.0, &[]));
}

#[test]
fn _02_extract_polygons_keeps_polygons_and_drops_lines() {
  assert_eq!(extract_polygons(Geometry::Polygon(Polygon::new(ring(0.0, 1.0), vec![]))).len(), 1);
  let two = Geometry::MultiPolygon(MultiPolygon(vec![
    Polygon::new(ring(0.0, 1.0), vec![]),
    Polygon::new(ring(2.0, 3.0), vec![]),
  ]));
  assert_eq!(extract_polygons(two).len(), 2);
  assert!(extract_polygons(Geometry::LineString(ring(0.0, 1.0))).is_empty());
}

#[test]
fn _03_run_chains_a_street_through_its_neighborhood_city_and_country() {
  let conn = crate::database::open_write(":memory:");
  batch_upsert(&conn, &nested_areas());
  let seen = RefCell::new(Vec::new());
  run(&conn, |p| seen.borrow_mut().push((p.total, p.processed)));

  assert_eq!(
    chain_of(&conn, way(STREET)),
    (
      vec![relation(NEIGHBORHOOD), relation(CITY), relation(COUNTRY)],
      "Rua X, Bairro, Cidade, Pais".to_string()
    )
  );
  assert_eq!(
    chain_of(&conn, relation(CITY)),
    (vec![relation(COUNTRY)], "Cidade, Pais".to_string())
  );
  assert_eq!(chain_of(&conn, relation(COUNTRY)), (vec![], "Pais".to_string()));
  let seen = seen.into_inner();
  assert_eq!(seen.first(), Some(&(Some(4), 0)), "the pending total comes first");
  assert_eq!(seen.last().map(|s| s.1), Some(4), "every row is reported");
}

#[test]
fn _04_a_street_outside_every_neighborhood_attaches_to_its_city() {
  let mut rows = nested_areas();
  rows.push(street(11, "Rua Y", 18.0));
  let conn = resolved(&rows);

  assert_eq!(
    chain_of(&conn, way(11)),
    (vec![relation(CITY), relation(COUNTRY)], "Rua Y, Cidade, Pais".to_string())
  );
}

#[test]
fn _05_the_larger_of_two_same_level_areas_contains_the_smaller() {
  let conn = resolved(&[
    area(COUNTRY, level::country, "Pais", 0.0, 100.0),
    area(2, level::city, "Grande", 0.0, 50.0),
    area(3, level::city, "Pequena", 10.0, 20.0),
  ]);

  assert_eq!(
    chain_of(&conn, relation(3)),
    (vec![relation(2), relation(COUNTRY)], "Pequena, Grande, Pais".to_string())
  );
  assert_eq!(chain_of(&conn, relation(2)), (vec![relation(COUNTRY)], "Grande, Pais".to_string()));
}

#[test]
fn _06_a_second_run_finds_nothing_pending() {
  let conn = resolved(&nested_areas());
  let seen = RefCell::new(Vec::new());
  run(&conn, |p| seen.borrow_mut().push((p.total, p.processed)));

  assert_eq!(seen.into_inner(), vec![(Some(0), 0)]);
  assert_eq!(count(&conn), 4);
}

#[test]
fn _07_a_file_database_takes_the_parallel_path_with_the_same_rows() {
  let guard = tempdir_guard::new("hierarchy_parallel");
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  let conn = crate::database::open_write(&db);
  let mut rows = nested_areas();
  rows.push(street(11, "Rua Y", 18.0));
  batch_upsert(&conn, &rows);
  run(&conn, |_| {});

  assert_eq!(count(&conn), 5);
  assert_eq!(
    chain_of(&conn, way(STREET)).0,
    vec![relation(NEIGHBORHOOD), relation(CITY), relation(COUNTRY)]
  );
  assert_eq!(chain_of(&conn, way(11)).1, "Rua Y, Cidade, Pais");
}
