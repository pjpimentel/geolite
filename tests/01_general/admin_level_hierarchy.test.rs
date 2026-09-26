use crate::admin_level::index_at;
use crate::common::harness::{open_sqlite_at, world};
use crate::extract::{copy_fixture, stage};
use crate::general::world;
use crate::street_merge::{parents_of, way};
use geo::{BoundingRect, Coord, Geometry, LineString, MultiPolygon, Polygon, coord};
use geozero::{CoordDimensions, ToWkb};

const COUNTRY: u64 = 1;

fn relation(osm_id: u64) -> i64 {
  ((osm_id << 1) | 1) as i64
}

struct row {
  id: i64,
  relation_id: Option<i64>,
  way_id: Option<i64>,
  level: u8,
  name: &'static str,
  geometry: Geometry<f64>,
}

fn area(osm_id: u64, level: u8, name: &'static str, min: [f64; 2], max: [f64; 2]) -> row {
  let ring = LineString(vec![
    coord! { x: min[0], y: min[1] },
    coord! { x: max[0], y: min[1] },
    coord! { x: max[0], y: max[1] },
    coord! { x: min[0], y: max[1] },
    coord! { x: min[0], y: min[1] },
  ]);
  row {
    id: relation(osm_id),
    relation_id: Some(osm_id as i64),
    way_id: None,
    level,
    name,
    geometry: Geometry::MultiPolygon(MultiPolygon(vec![Polygon::new(ring, vec![])])),
  }
}

fn street(osm_id: u64, name: &'static str, points: &[[f64; 2]]) -> row {
  let line: Vec<Coord<f64>> = points.iter().map(|p| coord! { x: p[0], y: p[1] }).collect();
  row {
    id: way(osm_id),
    relation_id: None,
    way_id: Some(osm_id as i64),
    level: 12,
    name,
    geometry: Geometry::LineString(LineString(line)),
  }
}

fn wkb(geometry: &Geometry<f64>) -> Vec<u8> {
  let bbox = geometry
    .bounding_rect()
    .expect("a synthetic geometry has a box");
  geometry
    .to_spatialite_wkb(
      CoordDimensions::default(),
      Some(4326),
      vec![bbox.min().x, bbox.min().y, bbox.max().x, bbox.max().y],
    )
    .expect("failed to encode a synthetic geometry")
}

// the chunk index is the cheapest stage that opens the database for writing, which is what creates
// the schema the synthetic rows go into
fn resolved(w: &world, name: &str, rows: &[row]) -> rusqlite::Connection {
  const SQL_INSERT: &str = "
    INSERT INTO admin_levels (
      id,
      relation_id,
      way_id,
      admin_level,
      wkb,
      name
    ) VALUES (
      ?1,
      ?2,
      ?3,
      ?4,
      ?5,
      ?6
    )
  ";

  let dir = w.scratch(name);
  let pbf = copy_fixture(w, &dir, "santos.osm.pbf");
  stage(w, &dir, &["exec", "extract-osm-pbf-blob-chunks", &pbf]);
  {
    let conn = rusqlite::Connection::open(dir.join("database.sqlite3"))
      .expect("failed to open the scratch database for writing");
    for row in rows {
      conn
        .execute(
          SQL_INSERT,
          rusqlite::params![
            row.id,
            row.relation_id,
            row.way_id,
            row.level,
            wkb(&row.geometry),
            row.name
          ],
        )
        .expect("failed to insert a synthetic row");
    }
  }
  index_at(w, &dir, "admin-levels-hierarchy");
  open_sqlite_at(&dir.join("database.sqlite3"))
}

// 00.00. half of the samples nests an area under a peer of its level: 32 of 65 stay beside it,
// 33 go under it
#[test]
#[ignore]
fn _00_00_half_of_the_samples_nests_an_area_under_a_peer() {
  let w = world();
  let conn = resolved(
    w,
    "hierarchy_nesting_threshold",
    &[
      area(COUNTRY, 2, "Country", [-10.0, -10.0], [20.0, 10.0]),
      area(2, 10, "Beside", [0.0, 0.0], [1.6, 1.6]),
      area(3, 10, "Larger Beside", [-2.0, -2.0], [3.6, 0.75]),
      area(4, 10, "Within", [10.0, 0.0], [11.6, 1.6]),
      area(5, 10, "Larger Around", [8.0, -2.0], [13.6, 0.85]),
    ],
  );

  assert_eq!(
    parents_of(&conn, relation(2)),
    vec![relation(COUNTRY)],
    "four rows of the grid, 32 of 65 samples, inside the peer: beside it"
  );
  assert_eq!(
    parents_of(&conn, relation(4)),
    vec![relation(5)],
    "four rows and the centroid, 33 of 65 samples, inside the peer: under it"
  );
  for larger in [3, 5] {
    assert_eq!(parents_of(&conn, relation(larger)), vec![relation(COUNTRY)]);
  }
}

// 00.01. a tenth of the samples straddles a more general area: 7 of 65 hang the area from the
// city, 6 leave it beside
#[test]
#[ignore]
fn _00_01_a_tenth_of_the_samples_straddles_a_more_general_area() {
  let w = world();
  let conn = resolved(
    w,
    "hierarchy_straddle_threshold",
    &[
      area(COUNTRY, 2, "Country", [-10.0, -10.0], [20.0, 10.0]),
      area(2, 10, "Over The Corner", [0.0, 0.0], [1.6, 1.6]),
      area(3, 8, "City Under Seven Samples", [-2.0, -2.0], [1.4, 0.2]),
      area(4, 10, "Short Of The Corner", [10.0, 0.0], [11.6, 1.6]),
      area(5, 8, "City Under Six Samples", [8.0, -2.0], [10.6, 0.4]),
    ],
  );

  assert_eq!(
    parents_of(&conn, relation(2)),
    vec![relation(3)],
    "seven columns of one row, 7 of 65 samples, inside the city: under it"
  );
  assert_eq!(
    parents_of(&conn, relation(4)),
    vec![relation(COUNTRY)],
    "three columns of two rows, 6 of 65 samples, inside the city: beside it"
  );
  for city in [3, 5] {
    assert_eq!(parents_of(&conn, relation(city)), vec![relation(COUNTRY)]);
  }
}

// 00.02. one sample inside hangs a street from an area, and a border only promotes the street to
// an area more specific than any it entered
#[test]
#[ignore]
fn _00_02_one_sample_inside_hangs_a_street_and_a_border_promotes_only_deeper() {
  let w = world();
  let conn = resolved(
    w,
    "hierarchy_street_threshold",
    &[
      area(COUNTRY, 2, "Country", [-10.0, -10.0], [20.0, 10.0]),
      area(2, 8, "City", [-2.0, -2.0], [5.0, 5.0]),
      area(3, 10, "Entered", [0.0, 0.0], [1.6, 1.6]),
      area(4, 10, "Touched", [1.6, 0.0], [3.2, 1.6]),
      street(
        10,
        "Long Street",
        &[
          [0.8, 0.8],
          [0.8, 4.0],
          [1.0, 4.0],
          [1.2, 4.0],
          [1.4, 4.0],
          [1.6, 4.0],
        ],
      ),
      street(11, "Short Street", &[[0.8, 0.8], [1.6, 0.8]]),
      street(12, "Border Street", &[[-1.0, 0.8], [0.0, 0.8]]),
    ],
  );

  assert_eq!(
    parents_of(&conn, way(10)),
    vec![relation(3)],
    "one of eleven samples inside the neighbourhood is enough"
  );
  assert_eq!(
    parents_of(&conn, way(11)),
    vec![relation(3)],
    "the border of a peer of the neighbourhood entered is only touched"
  );
  assert_eq!(
    parents_of(&conn, way(12)),
    vec![relation(3)],
    "the border of a neighbourhood, deeper than the city entered, promotes the street into it"
  );
}
