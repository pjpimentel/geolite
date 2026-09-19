use std::cell::RefCell;

use geo::{Geometry, MultiPolygon, Polygon};
use rusqlite::Connection;

use super::super::fixtures::{area, area_at, relation, ring, street, street_between, way};
use super::super::paths::paths_of;
use super::super::repository::{ancestry_of, count, parents_of, roots};
use super::{
  classify_in_polygons, classify_point, extract_polygons, point_class, polygon_entry, ring, run,
};
use crate::admin_level::repository::batch_upsert;
use crate::admin_level::{admin_level, level};
use crate::osm_pbf_file::pbf_fixtures::tempdir_guard;

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

fn resolved(rows: &[admin_level]) -> Connection {
  let conn = crate::database::open_write(":memory:");
  batch_upsert(&conn, rows);
  run(&conn, |_| {});
  conn
}

#[test]
fn _00_classify_point_tells_inside_from_the_border_and_from_outside() {
  let square = ring::new(vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 0.0]]);
  assert_eq!(classify_point(0.5, 0.5, &square), point_class::inside);
  assert_eq!(classify_point(2.0, 2.0, &square), point_class::outside);
  assert_eq!(classify_point(0.5, 0.0, &square), point_class::on_edge, "on a segment");
  assert_eq!(classify_point(1.0, 1.0, &square), point_class::on_edge, "a shared vertex");
  assert_eq!(
    classify_point(0.5, 0.5, &ring::new(vec![[0.0, 0.0], [1.0, 0.0]])),
    point_class::outside,
    "two points are not a ring"
  );
}

// 00.01: a ring long enough to be indexed answers the same as one walked edge by edge
#[test]
fn _00_01_an_indexed_ring_answers_like_a_walked_one() {
  let sides = super::RING_INDEX_MIN_EDGES + 8;
  let circle: Vec<[f64; 2]> = (0..=sides)
    .map(|i| {
      let angle = std::f64::consts::TAU * i as f64 / sides as f64;
      [angle.cos(), angle.sin()]
    })
    .collect();
  let indexed = ring::new(circle.clone());
  assert!(!indexed.bands.is_empty(), "the ring is long enough to be indexed");

  assert_eq!(classify_point(0.0, 0.0, &indexed), point_class::inside);
  assert_eq!(classify_point(5.0, 0.0, &indexed), point_class::outside);
  assert_eq!(classify_point(0.0, 5.0, &indexed), point_class::outside, "above the ring");
  assert_eq!(classify_point(circle[3][0], circle[3][1], &indexed), point_class::on_edge);
  for point in [[0.5, 0.5], [-0.9, 0.0], [0.99, 0.0], [1.01, 0.0], [0.7, 0.7]] {
    assert_eq!(
      classify_point(point[0], point[1], &indexed),
      classify_point(point[0], point[1], &ring::new(circle.clone())),
      "point {point:?}"
    );
  }
}

#[test]
fn _01_a_hole_excludes_its_interior_and_its_ring_is_a_border() {
  let poly = polygon_entry {
    exterior: ring::new(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [0.0, 0.0]]),
    interiors: vec![ring::new(vec![
      [4.0, 4.0],
      [6.0, 4.0],
      [6.0, 6.0],
      [4.0, 6.0],
      [4.0, 4.0],
    ])],
  };
  let one = std::slice::from_ref(&poly);
  assert_eq!(classify_in_polygons(1.0, 1.0, one), point_class::inside);
  assert_eq!(classify_in_polygons(5.0, 5.0, one), point_class::outside, "inside the hole");
  assert_eq!(classify_in_polygons(5.0, 4.0, one), point_class::on_edge, "on the hole's ring");
  assert_eq!(classify_in_polygons(1.0, 1.0, &[]), point_class::outside);
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
fn _03_run_links_a_street_to_its_neighborhood_and_each_area_to_the_one_around_it() {
  let conn = crate::database::open_write(":memory:");
  batch_upsert(&conn, &nested_areas());
  let seen = RefCell::new(Vec::new());
  run(&conn, |p| seen.borrow_mut().push((p.total, p.processed)));

  assert_eq!(parents_of(&conn, way(STREET)), vec![relation(NEIGHBORHOOD)]);
  assert_eq!(parents_of(&conn, relation(NEIGHBORHOOD)), vec![relation(CITY)]);
  assert_eq!(parents_of(&conn, relation(CITY)), vec![relation(COUNTRY)]);
  assert!(parents_of(&conn, relation(COUNTRY)).is_empty());
  let root_ids: Vec<i64> = roots(&conn).iter().map(|n| n.id).collect();
  assert_eq!(root_ids, vec![relation(COUNTRY)]);
  let seen = seen.into_inner();
  assert_eq!(seen.first(), Some(&(Some(4), 0)), "the pending total comes first");
  assert_eq!(seen.last().map(|s| s.1), Some(4), "every area is reported");
}

#[test]
fn _04_a_street_outside_every_neighborhood_attaches_to_its_city() {
  let mut rows = nested_areas();
  rows.push(street(11, "Rua Y", 18.0));
  let conn = resolved(&rows);

  assert_eq!(parents_of(&conn, way(11)), vec![relation(CITY)]);
}

#[test]
fn _05_the_larger_of_two_same_level_areas_contains_the_smaller() {
  let conn = resolved(&[
    area(COUNTRY, level::country, "Pais", 0.0, 100.0),
    area(2, level::city, "Grande", 0.0, 50.0),
    area(3, level::city, "Pequena", 10.0, 20.0),
  ]);

  assert_eq!(parents_of(&conn, relation(3)), vec![relation(2)]);
  assert_eq!(parents_of(&conn, relation(2)), vec![relation(COUNTRY)]);
}

#[test]
fn _06_a_second_run_finds_nothing_pending() {
  let conn = resolved(&nested_areas());
  let seen = RefCell::new(Vec::new());
  run(&conn, |p| seen.borrow_mut().push((p.total, p.processed)));

  assert_eq!(seen.into_inner(), vec![(Some(0), 0)]);
  assert_eq!(count(&conn), 4, "one edge per area, the root's included");
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
  assert_eq!(parents_of(&conn, way(STREET)), vec![relation(NEIGHBORHOOD)]);
  assert_eq!(parents_of(&conn, way(11)), vec![relation(CITY)]);
}

const COUNTRY_BOX: ([f64; 2], [f64; 2]) = ([0.0, 0.0], [100.0, 100.0]);

fn country() -> admin_level {
  area_at(COUNTRY, level::country, "Pais", COUNTRY_BOX.0, COUNTRY_BOX.1)
}

fn paths_from(conn: &Connection, id: i64) -> Vec<Vec<i64>> {
  paths_of(id, &ancestry_of(conn, &[id]))
}

#[test]
fn _08_a_street_crossing_two_neighbourhoods_gets_both_as_parents_in_id_order() {
  let conn = resolved(&[
    country(),
    area_at(2, level::city, "Cidade", [0.0, 0.0], [20.0, 10.0]),
    area_at(3, level::neighborhood, "Bairro A", [0.0, 0.0], [10.0, 10.0]),
    area_at(4, level::neighborhood, "Bairro B", [10.0, 0.0], [20.0, 10.0]),
    street_between(10, "Rua Larga", [5.0, 5.0], [15.0, 5.0]),
  ]);

  assert_eq!(parents_of(&conn, way(10)), vec![relation(3), relation(4)]);
  assert_eq!(
    paths_from(&conn, way(10)),
    vec![
      vec![relation(3), relation(2), relation(COUNTRY)],
      vec![relation(4), relation(2), relation(COUNTRY)]
    ],
    "one path per parent, each up to the country"
  );
}

#[test]
fn _09_a_street_along_a_shared_boundary_hangs_from_both_neighbourhoods() {
  let conn = resolved(&[
    country(),
    area_at(2, level::city, "Cidade", [0.0, 0.0], [20.0, 10.0]),
    area_at(3, level::neighborhood, "Bairro A", [0.0, 0.0], [10.0, 10.0]),
    area_at(4, level::neighborhood, "Bairro B", [10.0, 0.0], [20.0, 10.0]),
    // traced over the line the two neighbourhoods share, as osm draws a boundary street
    street_between(10, "Rua da Divisa", [10.0, 2.0], [10.0, 8.0]),
  ]);

  assert_eq!(parents_of(&conn, way(10)), vec![relation(3), relation(4)]);
}

#[test]
fn _10_a_street_that_only_ends_on_a_boundary_stays_in_the_area_it_runs_through() {
  let conn = resolved(&[
    country(),
    area_at(2, level::city, "Cidade", [0.0, 0.0], [20.0, 10.0]),
    area_at(3, level::neighborhood, "Bairro A", [0.0, 0.0], [10.0, 10.0]),
    area_at(4, level::neighborhood, "Bairro B", [10.0, 0.0], [20.0, 10.0]),
    street_between(10, "Rua Curta", [5.0, 5.0], [10.0, 5.0]),
  ]);

  assert_eq!(parents_of(&conn, way(10)), vec![relation(3)], "the neighbour is only touched");
}

#[test]
fn _11_a_neighbourhood_across_two_cities_fans_its_streets_out() {
  let conn = resolved(&[
    country(),
    area_at(2, level::city, "Cidade A", [0.0, 0.0], [10.0, 10.0]),
    area_at(3, level::city, "Cidade B", [10.0, 0.0], [20.0, 10.0]),
    area_at(4, level::neighborhood, "Bairro", [5.0, 2.0], [15.0, 8.0]),
    street_between(10, "Rua Interna", [7.0, 5.0], [8.0, 5.0]),
  ]);

  assert_eq!(parents_of(&conn, relation(4)), vec![relation(2), relation(3)]);
  assert_eq!(parents_of(&conn, way(10)), vec![relation(4)], "one parent");
  assert_eq!(
    paths_from(&conn, way(10)),
    vec![
      vec![relation(4), relation(2), relation(COUNTRY)],
      vec![relation(4), relation(3), relation(COUNTRY)]
    ],
    "two paths: the neighbourhood is in two cities"
  );
}

#[test]
fn _12_a_road_leaving_its_neighbourhood_into_the_next_city_keeps_both_branches() {
  let conn = resolved(&[
    country(),
    area_at(2, level::city, "Cidade A", [0.0, 0.0], [50.0, 100.0]),
    area_at(3, level::city, "Cidade B", [50.0, 0.0], [100.0, 100.0]),
    area_at(4, level::neighborhood, "Bairro", [10.0, 40.0], [20.0, 60.0]),
    street_between(10, "Rodovia", [15.0, 50.0], [70.0, 50.0]),
  ]);

  assert_eq!(
    parents_of(&conn, way(10)),
    vec![relation(3), relation(4)],
    "the neighbourhood it enters and the city it runs on into"
  );
}

#[test]
fn _13_a_same_level_area_nests_only_when_mostly_inside() {
  let conn = resolved(&[
    country(),
    area_at(2, level::neighborhood, "Grande", [0.0, 0.0], [10.0, 10.0]),
    area_at(3, level::neighborhood, "Dentro", [2.0, 2.0], [4.0, 4.0]),
    area_at(4, level::neighborhood, "Ao Lado", [8.0, 0.0], [16.0, 10.0]),
  ]);

  assert_eq!(parents_of(&conn, relation(3)), vec![relation(2)], "wholly inside");
  assert_eq!(
    parents_of(&conn, relation(4)),
    vec![relation(COUNTRY)],
    "a quarter inside is a neighbour, not a child"
  );
}

#[test]
fn _14_nested_same_level_areas_reduce_to_the_innermost() {
  let conn = resolved(&[
    country(),
    area_at(2, level::neighborhood, "Externo", [0.0, 0.0], [20.0, 20.0]),
    area_at(3, level::neighborhood, "Interno", [5.0, 5.0], [10.0, 10.0]),
    street_between(10, "Rua Dentro", [6.0, 6.0], [7.0, 7.0]),
  ]);

  assert_eq!(parents_of(&conn, way(10)), vec![relation(3)], "only the innermost");
  assert_eq!(parents_of(&conn, relation(3)), vec![relation(2)]);
}

#[test]
fn _15_the_file_and_memory_paths_write_the_same_edges() {
  let rows = [
    country(),
    area_at(2, level::city, "Cidade", [0.0, 0.0], [20.0, 10.0]),
    area_at(3, level::neighborhood, "Bairro A", [0.0, 0.0], [10.0, 10.0]),
    area_at(4, level::neighborhood, "Bairro B", [10.0, 0.0], [20.0, 10.0]),
    street_between(10, "Rua Larga", [5.0, 5.0], [15.0, 5.0]),
    street_between(11, "Rua Curta", [5.0, 5.0], [10.0, 5.0]),
  ];
  let guard = tempdir_guard::new("hierarchy_same_edges");
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  let on_file = crate::database::open_write(&db);
  batch_upsert(&on_file, &rows);
  run(&on_file, |_| {});

  assert_eq!(edges_of(&on_file), edges_of(&resolved(&rows)));
}

fn edges_of(conn: &Connection) -> Vec<(i64, Option<i64>)> {
  conn
    .prepare("SELECT admin_level_id, parent_id FROM admin_levels_hierarchy ORDER BY 1, 2")
    .expect("failed to prepare")
    .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read an edge"))
    .collect()
}
