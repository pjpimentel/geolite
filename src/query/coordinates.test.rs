use super::*;
use crate::database::admin_levels::{admin_levels as admin_levels_row, batch_upsert};
use geo::{Coord, Polygon};

const SQL_UPDATE_WKB: &str = "
  UPDATE admin_levels
  SET wkb = ?1
  WHERE id = ?2
";

struct street_data {
  name: &'static str,
  lon1: f64,
  lat1: f64,
  lon2: f64,
  lat2: f64,
}

const STREETS: &[street_data] = &[
  street_data {
    name: "street_00",
    lon1: -46.31980,
    lat1: -23.97241,
    lon2: -46.31979,
    lat2: -23.97240,
  },
  street_data {
    name: "street_01",
    lon1: -46.31970,
    lat1: -23.97231,
    lon2: -46.31969,
    lat2: -23.97230,
  },
  street_data {
    name: "street_02",
    lon1: -46.31960,
    lat1: -23.97221,
    lon2: -46.31959,
    lat2: -23.97220,
  },
  street_data {
    name: "street_03",
    lon1: -46.31950,
    lat1: -23.97211,
    lon2: -46.31949,
    lat2: -23.97210,
  },
  street_data {
    name: "street_04",
    lon1: -46.31940,
    lat1: -23.97201,
    lon2: -46.31939,
    lat2: -23.97200,
  },
  street_data {
    name: "street_05",
    lon1: -46.31930,
    lat1: -23.97191,
    lon2: -46.31929,
    lat2: -23.97190,
  },
  street_data {
    name: "street_06",
    lon1: -46.31920,
    lat1: -23.97181,
    lon2: -46.31919,
    lat2: -23.97180,
  },
  street_data {
    name: "street_07",
    lon1: -46.31910,
    lat1: -23.97171,
    lon2: -46.31909,
    lat2: -23.97170,
  },
  street_data {
    name: "street_08",
    lon1: -46.31900,
    lat1: -23.97161,
    lon2: -46.31899,
    lat2: -23.97160,
  },
  street_data {
    name: "street_09",
    lon1: -46.31890,
    lat1: -23.97151,
    lon2: -46.31889,
    lat2: -23.97150,
  },
  street_data {
    name: "street_10",
    lon1: -46.31880,
    lat1: -23.97141,
    lon2: -46.31879,
    lat2: -23.97140,
  },
  street_data {
    name: "street_11",
    lon1: -46.31870,
    lat1: -23.97131,
    lon2: -46.31869,
    lat2: -23.97130,
  },
  street_data {
    name: "street_12",
    lon1: -46.31860,
    lat1: -23.97121,
    lon2: -46.31859,
    lat2: -23.97120,
  },
  street_data {
    name: "street_13",
    lon1: -46.31850,
    lat1: -23.97111,
    lon2: -46.31849,
    lat2: -23.97110,
  },
  street_data {
    name: "street_14",
    lon1: -46.31840,
    lat1: -23.97101,
    lon2: -46.31839,
    lat2: -23.97100,
  },
  street_data {
    name: "street_15",
    lon1: -46.31830,
    lat1: -23.97091,
    lon2: -46.31829,
    lat2: -23.97090,
  },
  street_data {
    name: "street_16",
    lon1: -46.31820,
    lat1: -23.97081,
    lon2: -46.31819,
    lat2: -23.97080,
  },
  street_data {
    name: "street_17",
    lon1: -46.31810,
    lat1: -23.97071,
    lon2: -46.31809,
    lat2: -23.97070,
  },
  street_data {
    name: "street_18",
    lon1: -46.31800,
    lat1: -23.97061,
    lon2: -46.31799,
    lat2: -23.97060,
  },
  street_data {
    name: "street_19",
    lon1: -46.31790,
    lat1: -23.97051,
    lon2: -46.31789,
    lat2: -23.97050,
  },
  street_data {
    name: "street_20",
    lon1: -46.31780,
    lat1: -23.97041,
    lon2: -46.31779,
    lat2: -23.97040,
  },
  street_data {
    name: "street_21",
    lon1: -46.31770,
    lat1: -23.97031,
    lon2: -46.31769,
    lat2: -23.97030,
  },
  street_data {
    name: "street_22",
    lon1: -46.31760,
    lat1: -23.97021,
    lon2: -46.31759,
    lat2: -23.97020,
  },
  street_data {
    name: "street_23",
    lon1: -46.31750,
    lat1: -23.97011,
    lon2: -46.31749,
    lat2: -23.97010,
  },
  street_data {
    name: "street_24",
    lon1: -46.31740,
    lat1: -23.97001,
    lon2: -46.31739,
    lat2: -23.97000,
  },
];

fn make_street_row(s: &street_data, way_id: u64) -> admin_levels_row {
  let ls = LineString(vec![
    Coord {
      x: s.lon1,
      y: s.lat1,
    },
    Coord {
      x: s.lon2,
      y: s.lat2,
    },
  ]);
  admin_levels_row {
    relation_id: None,
    way_id: Some(way_id),
    admin_level: 12,
    wkb: Geometry::LineString(ls).into(),
    name: s.name.to_string(),
    country_iso_code: None,
    post_code: None,
  }
}

#[test]
fn _00_returns_the_10_closest_records() {
  let conn = crate::database::open_write(":memory:");
  let rows: Vec<admin_levels_row> = STREETS
    .iter()
    .enumerate()
    .map(|(i, s)| make_street_row(s, (i + 1) as u64))
    .collect();
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});

  let output = crate::query::run(
    &conn,
    None,
    "-23.97241,-46.31980",
    None,
    None,
    None,
    None,
    true,
  );
  assert_eq!(output.matches.len(), 10);

  let m0 = &output.matches[0];
  assert_eq!(m0.admin_levels.len(), 1);
  assert_eq!(m0.admin_levels[0].level, 12);
  assert_eq!(m0.admin_levels[0].name, "street_00");
  assert_eq!(m0.admin_levels[0].osm_relation_id, None);
  assert_eq!(m0.admin_levels[0].osm_way_id, Some(1));
  assert_eq!(m0.coordinates_distance_in_meters, Some(0));
  assert_eq!(m0.latitude, -23.97241);
  assert_eq!(m0.longitude, -46.31980);
  assert_eq!(m0.attributes.country_iso_3166_1_alpha_2_code, None);
  assert_eq!(m0.attributes.post_code, None);
  assert_eq!(m0.id, 2);
  assert_eq!(m0.similarity, None);
  assert_eq!(m0.friendly_name, "street_00");

  let m9 = &output.matches[9];
  assert_eq!(m9.admin_levels.len(), 1);
  assert_eq!(m9.admin_levels[0].level, 12);
  assert_eq!(m9.admin_levels[0].name, "street_09");
  assert_eq!(m9.admin_levels[0].osm_relation_id, None);
  assert_eq!(m9.admin_levels[0].osm_way_id, Some(10));
}

#[test]
fn _01_results_are_ordered_by_distance_from_coordinates_to_street() {
  let conn = crate::database::open_write(":memory:");
  let streets = [
    street_data {
      name: "street_far",
      lon1: -46.31980,
      lat1: -23.97151,
      lon2: -46.31979,
      lat2: -23.97150,
    },
    street_data {
      name: "street_mid",
      lon1: -46.31980,
      lat1: -23.97196,
      lon2: -46.31979,
      lat2: -23.97195,
    },
    street_data {
      name: "street_near",
      lon1: -46.31980,
      lat1: -23.97232,
      lon2: -46.31979,
      lat2: -23.97231,
    },
  ];
  let rows: Vec<admin_levels_row> = streets
    .iter()
    .enumerate()
    .map(|(i, s)| make_street_row(s, (i + 1) as u64))
    .collect();
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});

  let output = crate::query::run(
    &conn,
    None,
    "-23.97241,-46.31980",
    None,
    None,
    None,
    None,
    true,
  );
  assert_eq!(output.matches.len(), 3);

  assert_eq!(output.matches[0].admin_levels.len(), 1);
  assert_eq!(output.matches[0].admin_levels[0].level, 12);
  assert_eq!(output.matches[0].admin_levels[0].name, "street_near");
  assert_eq!(output.matches[0].admin_levels[0].osm_relation_id, None);
  assert_eq!(output.matches[0].admin_levels[0].osm_way_id, Some(3));

  assert_eq!(output.matches[1].admin_levels.len(), 1);
  assert_eq!(output.matches[1].admin_levels[0].level, 12);
  assert_eq!(output.matches[1].admin_levels[0].name, "street_mid");
  assert_eq!(output.matches[1].admin_levels[0].osm_relation_id, None);
  assert_eq!(output.matches[1].admin_levels[0].osm_way_id, Some(2));

  assert_eq!(output.matches[2].admin_levels.len(), 1);
  assert_eq!(output.matches[2].admin_levels[0].level, 12);
  assert_eq!(output.matches[2].admin_levels[0].name, "street_far");
  assert_eq!(output.matches[2].admin_levels[0].osm_relation_id, None);
  assert_eq!(output.matches[2].admin_levels[0].osm_way_id, Some(1));

  let d0 = output.matches[0].coordinates_distance_in_meters.unwrap();
  let d1 = output.matches[1].coordinates_distance_in_meters.unwrap();
  let d2 = output.matches[2].coordinates_distance_in_meters.unwrap();
  assert!(d0 < d1, "expected d0={} < d1={}", d0, d1);
  assert!(d1 < d2, "expected d1={} < d2={}", d1, d2);
}

fn run_hierarchy_case(area_admin_level: u8, area_name: &str) {
  let conn = crate::database::open_write(":memory:");

  let street_ls = LineString(vec![
    Coord {
      x: -46.31980,
      y: -23.97241,
    },
    Coord {
      x: -46.31979,
      y: -23.97240,
    },
  ]);
  let area_polygon = Polygon::new(
    LineString(vec![
      Coord { x: -47.0, y: -24.5 },
      Coord { x: -46.0, y: -24.5 },
      Coord { x: -46.0, y: -23.5 },
      Coord { x: -47.0, y: -23.5 },
      Coord { x: -47.0, y: -24.5 },
    ]),
    vec![],
  );

  let rows = vec![
    admin_levels_row {
      relation_id: None,
      way_id: Some(1),
      admin_level: 12,
      wkb: Geometry::LineString(street_ls).into(),
      name: "test_street".to_string(),
      country_iso_code: None,
      post_code: None,
    },
    admin_levels_row {
      relation_id: None,
      way_id: Some(2),
      admin_level: area_admin_level,
      wkb: Geometry::Polygon(area_polygon).into(),
      name: area_name.to_string(),
      country_iso_code: None,
      post_code: None,
    },
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});

  let output = crate::query::run(
    &conn,
    None,
    "-23.97241,-46.31980",
    None,
    None,
    None,
    None,
    true,
  );
  assert_eq!(output.matches.len(), 1);
  assert_eq!(output.matches[0].admin_levels.len(), 2);
  assert_eq!(output.matches[0].admin_levels[0].level, area_admin_level);
  assert_eq!(output.matches[0].admin_levels[0].name, area_name);
  assert_eq!(output.matches[0].admin_levels[0].osm_relation_id, None);
  assert_eq!(output.matches[0].admin_levels[0].osm_way_id, Some(2));
  assert_eq!(output.matches[0].admin_levels[1].level, 12);
  assert_eq!(output.matches[0].admin_levels[1].name, "test_street");
  assert_eq!(output.matches[0].admin_levels[1].osm_relation_id, None);
  assert_eq!(output.matches[0].admin_levels[1].osm_way_id, Some(1));
}

#[test]
fn _02_admin_level_hierarchy_is_resolved_and_returned() {
  run_hierarchy_case(1, "area_1");
  run_hierarchy_case(2, "area_2");
  run_hierarchy_case(3, "area_3");
  run_hierarchy_case(4, "area_4");
  run_hierarchy_case(5, "area_5");
  run_hierarchy_case(6, "area_6");
  run_hierarchy_case(7, "area_7");
  run_hierarchy_case(8, "area_8");
  run_hierarchy_case(9, "area_9");
  run_hierarchy_case(10, "area_10");
  run_hierarchy_case(11, "area_11");
}

fn run_split_hierarchy_case(area_admin_level: u8) {
  let conn = crate::database::open_write(":memory:");

  let street_a_ls = LineString(vec![
    Coord {
      x: -46.31980,
      y: -23.97242,
    },
    Coord {
      x: -46.31979,
      y: -23.97241,
    },
  ]);
  let street_b_ls = LineString(vec![
    Coord {
      x: -46.31980,
      y: -23.97255,
    },
    Coord {
      x: -46.31979,
      y: -23.97254,
    },
  ]);
  let area_a_poly = Polygon::new(
    LineString(vec![
      Coord {
        x: -47.0,
        y: -23.97248,
      },
      Coord {
        x: -46.0,
        y: -23.97248,
      },
      Coord {
        x: -46.0,
        y: -23.97,
      },
      Coord {
        x: -47.0,
        y: -23.97,
      },
      Coord {
        x: -47.0,
        y: -23.97248,
      },
    ]),
    vec![],
  );
  let area_b_poly = Polygon::new(
    LineString(vec![
      Coord { x: -47.0, y: -24.5 },
      Coord { x: -46.0, y: -24.5 },
      Coord {
        x: -46.0,
        y: -23.97248,
      },
      Coord {
        x: -47.0,
        y: -23.97248,
      },
      Coord { x: -47.0, y: -24.5 },
    ]),
    vec![],
  );

  let rows = vec![
    admin_levels_row {
      relation_id: None,
      way_id: Some(1),
      admin_level: 12,
      wkb: Geometry::LineString(street_a_ls).into(),
      name: "street_a".to_string(),
      country_iso_code: None,
      post_code: None,
    },
    admin_levels_row {
      relation_id: None,
      way_id: Some(2),
      admin_level: 12,
      wkb: Geometry::LineString(street_b_ls).into(),
      name: "street_b".to_string(),
      country_iso_code: None,
      post_code: None,
    },
    admin_levels_row {
      relation_id: None,
      way_id: Some(3),
      admin_level: area_admin_level,
      wkb: Geometry::Polygon(area_a_poly).into(),
      name: "area_a".to_string(),
      country_iso_code: None,
      post_code: None,
    },
    admin_levels_row {
      relation_id: None,
      way_id: Some(4),
      admin_level: area_admin_level,
      wkb: Geometry::Polygon(area_b_poly).into(),
      name: "area_b".to_string(),
      country_iso_code: None,
      post_code: None,
    },
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});

  let output = crate::query::run(
    &conn,
    None,
    "-23.97241,-46.31980",
    None,
    None,
    None,
    None,
    true,
  );
  assert_eq!(output.matches.len(), 2);

  assert_eq!(output.matches[0].admin_levels.len(), 2);
  assert_eq!(output.matches[0].admin_levels[0].level, area_admin_level);
  assert_eq!(output.matches[0].admin_levels[0].name, "area_a");
  assert_eq!(output.matches[0].admin_levels[0].osm_relation_id, None);
  assert_eq!(output.matches[0].admin_levels[0].osm_way_id, Some(3));
  assert_eq!(output.matches[0].admin_levels[1].level, 12);
  assert_eq!(output.matches[0].admin_levels[1].name, "street_a");
  assert_eq!(output.matches[0].admin_levels[1].osm_relation_id, None);
  assert_eq!(output.matches[0].admin_levels[1].osm_way_id, Some(1));

  assert_eq!(output.matches[1].admin_levels.len(), 2);
  assert_eq!(output.matches[1].admin_levels[0].level, area_admin_level);
  assert_eq!(output.matches[1].admin_levels[0].name, "area_b");
  assert_eq!(output.matches[1].admin_levels[0].osm_relation_id, None);
  assert_eq!(output.matches[1].admin_levels[0].osm_way_id, Some(4));
  assert_eq!(output.matches[1].admin_levels[1].level, 12);
  assert_eq!(output.matches[1].admin_levels[1].name, "street_b");
  assert_eq!(output.matches[1].admin_levels[1].osm_relation_id, None);
  assert_eq!(output.matches[1].admin_levels[1].osm_way_id, Some(2));
}

#[test]
fn _03_each_street_keeps_its_own_hierarchy_when_multiple_streets_match() {
  run_split_hierarchy_case(1);
  run_split_hierarchy_case(2);
  run_split_hierarchy_case(3);
  run_split_hierarchy_case(4);
  run_split_hierarchy_case(5);
  run_split_hierarchy_case(6);
  run_split_hierarchy_case(7);
  run_split_hierarchy_case(8);
  run_split_hierarchy_case(9);
  run_split_hierarchy_case(10);
  run_split_hierarchy_case(11);
}

fn run_nested_polygons_case(area_admin_level: u8) {
  let conn = crate::database::open_write(":memory:");

  let street_ls = LineString(vec![
    Coord {
      x: -46.31980,
      y: -23.97241,
    },
    Coord {
      x: -46.31979,
      y: -23.97240,
    },
  ]);
  let area_a_poly = Polygon::new(
    LineString(vec![
      Coord { x: -47.0, y: -24.5 },
      Coord { x: -46.0, y: -24.5 },
      Coord { x: -46.0, y: -23.5 },
      Coord { x: -47.0, y: -23.5 },
      Coord { x: -47.0, y: -24.5 },
    ]),
    vec![],
  );
  let area_b_poly = Polygon::new(
    LineString(vec![
      Coord {
        x: -46.35,
        y: -23.99,
      },
      Coord {
        x: -46.30,
        y: -23.99,
      },
      Coord {
        x: -46.30,
        y: -23.96,
      },
      Coord {
        x: -46.35,
        y: -23.96,
      },
      Coord {
        x: -46.35,
        y: -23.99,
      },
    ]),
    vec![],
  );
  let area_c_poly = Polygon::new(
    LineString(vec![
      Coord {
        x: -46.325,
        y: -23.975,
      },
      Coord {
        x: -46.315,
        y: -23.975,
      },
      Coord {
        x: -46.315,
        y: -23.970,
      },
      Coord {
        x: -46.325,
        y: -23.970,
      },
      Coord {
        x: -46.325,
        y: -23.975,
      },
    ]),
    vec![],
  );

  let rows = vec![
    admin_levels_row {
      relation_id: None,
      way_id: Some(1),
      admin_level: 12,
      wkb: Geometry::LineString(street_ls).into(),
      name: "test_street".to_string(),
      country_iso_code: None,
      post_code: None,
    },
    admin_levels_row {
      relation_id: None,
      way_id: Some(2),
      admin_level: area_admin_level,
      wkb: Geometry::Polygon(area_a_poly).into(),
      name: "area_a".to_string(),
      country_iso_code: None,
      post_code: None,
    },
    admin_levels_row {
      relation_id: None,
      way_id: Some(3),
      admin_level: area_admin_level,
      wkb: Geometry::Polygon(area_b_poly).into(),
      name: "area_b".to_string(),
      country_iso_code: None,
      post_code: None,
    },
    admin_levels_row {
      relation_id: None,
      way_id: Some(4),
      admin_level: area_admin_level,
      wkb: Geometry::Polygon(area_c_poly).into(),
      name: "area_c".to_string(),
      country_iso_code: None,
      post_code: None,
    },
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});

  let output = crate::query::run(
    &conn,
    None,
    "-23.97241,-46.31980",
    None,
    None,
    None,
    None,
    true,
  );
  assert_eq!(output.matches.len(), 1);
  assert_eq!(
    output.matches[0].friendly_name,
    "test_street, area_c, area_b, area_a"
  );
  assert_eq!(output.matches[0].admin_levels.len(), 4);
  assert_eq!(output.matches[0].admin_levels[0].level, area_admin_level);
  assert_eq!(output.matches[0].admin_levels[0].name, "area_a");
  assert_eq!(output.matches[0].admin_levels[0].osm_relation_id, None);
  assert_eq!(output.matches[0].admin_levels[0].osm_way_id, Some(2));
  assert_eq!(output.matches[0].admin_levels[1].level, area_admin_level);
  assert_eq!(output.matches[0].admin_levels[1].name, "area_b");
  assert_eq!(output.matches[0].admin_levels[1].osm_relation_id, None);
  assert_eq!(output.matches[0].admin_levels[1].osm_way_id, Some(3));
  assert_eq!(output.matches[0].admin_levels[2].level, area_admin_level);
  assert_eq!(output.matches[0].admin_levels[2].name, "area_c");
  assert_eq!(output.matches[0].admin_levels[2].osm_relation_id, None);
  assert_eq!(output.matches[0].admin_levels[2].osm_way_id, Some(4));
  assert_eq!(output.matches[0].admin_levels[3].level, 12);
  assert_eq!(output.matches[0].admin_levels[3].name, "test_street");
  assert_eq!(output.matches[0].admin_levels[3].osm_relation_id, None);
  assert_eq!(output.matches[0].admin_levels[3].osm_way_id, Some(1));
}

#[test]
fn _04_when_street_is_inside_three_polygons_at_same_level_all_are_grouped_and_ordered() {
  run_nested_polygons_case(1);
  run_nested_polygons_case(2);
  run_nested_polygons_case(3);
  run_nested_polygons_case(4);
  run_nested_polygons_case(5);
  run_nested_polygons_case(6);
  run_nested_polygons_case(7);
  run_nested_polygons_case(8);
  run_nested_polygons_case(9);
  run_nested_polygons_case(10);
  run_nested_polygons_case(11);
}

#[test]
fn _05_bounding_box_keeps_only_matches_inside_the_box() {
  let conn = crate::database::open_write(":memory:");
  let rows: Vec<admin_levels_row> = STREETS
    .iter()
    .enumerate()
    .map(|(i, s)| make_street_row(s, (i + 1) as u64))
    .collect();
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});

  let bbox = crate::query::bounding_box {
    min_lat: -23.97245,
    max_lat: -23.97235,
    min_lon: -46.31985,
    max_lon: -46.31975,
  };
  let bounds = crate::query::bounding_geometry::from_rect(bbox);
  let output = crate::query::run(
    &conn,
    None,
    "-23.97241,-46.31980",
    None,
    None,
    Some(bounds),
    None,
    true,
  );
  assert_eq!(output.matches.len(), 1);
  assert_eq!(output.matches[0].admin_levels[0].name, "street_00");
}

#[test]
fn _06_bounding_box_excluding_all_matches_returns_empty_matches() {
  let conn = crate::database::open_write(":memory:");
  let rows: Vec<admin_levels_row> = STREETS
    .iter()
    .enumerate()
    .map(|(i, s)| make_street_row(s, (i + 1) as u64))
    .collect();
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});

  let bbox = crate::query::bounding_box {
    min_lat: 10.0,
    max_lat: 11.0,
    min_lon: 10.0,
    max_lon: 11.0,
  };
  let bounds = crate::query::bounding_geometry::from_rect(bbox);
  let output = crate::query::run(
    &conn,
    None,
    "-23.97241,-46.31980",
    None,
    None,
    Some(bounds),
    None,
    true,
  );
  assert!(output.matches.is_empty());
}

#[test]
fn _07_degenerate_bounding_geometry_with_zero_area_does_not_panic() {
  let conn = crate::database::open_write(":memory:");
  let rows: Vec<admin_levels_row> = STREETS
    .iter()
    .enumerate()
    .map(|(i, s)| make_street_row(s, (i + 1) as u64))
    .collect();
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});

  let bbox = crate::query::bounding_box {
    min_lat: -23.97241,
    max_lat: -23.97241,
    min_lon: -46.31980,
    max_lon: -46.31980,
  };
  // geo::contains é boundary-exclusive: um poligono de area zero nao contem ponto algum
  let bounds = crate::query::bounding_geometry::from_rect(bbox);
  let output = crate::query::run(
    &conn,
    None,
    "-23.97241,-46.31980",
    None,
    None,
    Some(bounds),
    None,
    true,
  );
  assert!(output.matches.is_empty());
}

#[test]
fn _08_admin_level_filter_keeps_matches_with_requested_level() {
  let conn = crate::database::open_write(":memory:");
  let rows: Vec<admin_levels_row> = STREETS
    .iter()
    .enumerate()
    .map(|(i, s)| make_street_row(s, (i + 1) as u64))
    .collect();
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});

  let output = crate::query::run(
    &conn,
    None,
    "-23.97241,-46.31980",
    None,
    None,
    None,
    Some(vec![12]),
    true,
  );
  assert_eq!(output.matches.len(), 10);
}

#[test]
fn _09_admin_level_filter_with_no_matching_level_returns_empty_matches() {
  let conn = crate::database::open_write(":memory:");
  let rows: Vec<admin_levels_row> = STREETS
    .iter()
    .enumerate()
    .map(|(i, s)| make_street_row(s, (i + 1) as u64))
    .collect();
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});

  let output = crate::query::run(
    &conn,
    None,
    "-23.97241,-46.31980",
    None,
    None,
    None,
    Some(vec![8]),
    true,
  );
  assert!(output.matches.is_empty());
}

#[test]
fn _10_include_wkt_emits_wkt_per_admin_level() {
  // todo!()
}

#[test]
fn _11_bounding_wkt_clips_points_outside_polygon_but_inside_envelope() {
  // todo!()
}

#[test]
fn _12_bounding_wkt_keeps_in_polygon_match_ranked_beyond_max_results() {
  let conn = crate::database::open_write(":memory:");
  let mut rows: Vec<admin_levels_row> = Vec::new();
  for i in 1..=10 {
    let c = 0.001 * i as f64;
    let ls = LineString(vec![
      Coord { x: c, y: c },
      Coord {
        x: c + 0.0001,
        y: c + 0.0001,
      },
    ]);
    rows.push(admin_levels_row {
      relation_id: None,
      way_id: Some(i as u64),
      admin_level: 12,
      wkb: Geometry::LineString(ls).into(),
      name: format!("near_{i:02}"),
      country_iso_code: None,
      post_code: None,
    });
  }
  let far = LineString(vec![
    Coord { x: 0.05, y: 0.05 },
    Coord {
      x: 0.0501,
      y: 0.0501,
    },
  ]);
  rows.push(admin_levels_row {
    relation_id: None,
    way_id: Some(11),
    admin_level: 12,
    wkb: Geometry::LineString(far).into(),
    name: "far_street".to_string(),
    country_iso_code: None,
    post_code: None,
  });
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});

  // interior do triangulo: x+y > 0.06 → contem far (0.10), exclui as 10 near (≤0.02). o envelope
  // [-0.02,0.08]² cobre todas, entao as near passam o rtree e (sem a correcao) truncariam a far fora
  let bounds =
    crate::http::parse_bounding_wkt("POLYGON((0.08 0.08, -0.02 0.08, 0.08 -0.02, 0.08 0.08))")
      .unwrap();
  let output = crate::query::run(&conn, None, "0,0", None, None, Some(bounds), None, true);
  let names: Vec<&str> = output
    .matches
    .iter()
    .map(|m| m.admin_levels.last().unwrap().name.as_str())
    .collect();
  assert_eq!(names, vec!["far_street"]);
}

#[test]
fn _13_no_candidates_returns_empty() {
  let conn = crate::database::open_write(":memory:");
  crate::index::coordinates::run(&conn, |_| {});

  let candidates = best_admin_levels(&conn, Point::new(STREETS[0].lon1, STREETS[0].lat1), None);
  assert!(candidates.is_empty());
}

#[test]
fn _14_candidate_outside_rtree_delta_is_not_returned() {
  let conn = crate::database::open_write(":memory:");
  batch_upsert(&conn, &[make_street_row(&STREETS[0], 1)]);
  crate::index::coordinates::run(&conn, |_| {});

  // street_00 sits near (-46.3, -23.97); a query at (0, 0) is far beyond the
  // RTREE_DELTA_DEG (0.1) box, so the rtree pre-filter never surfaces it.
  let candidates = best_admin_levels(&conn, Point::new(0.0, 0.0), None);
  assert!(candidates.is_empty());
}

#[test]
fn _15_candidate_within_delta_is_included_with_distance() {
  let conn = crate::database::open_write(":memory:");
  batch_upsert(&conn, &[make_street_row(&STREETS[0], 1)]);
  crate::index::coordinates::run(&conn, |_| {});

  // querying exactly on the street's first endpoint → closest point is that
  // vertex → haversine distance is 0.
  let candidates = best_admin_levels(&conn, Point::new(STREETS[0].lon1, STREETS[0].lat1), None);
  assert_eq!(candidates.len(), 1);
  let c = &candidates[0];
  assert_eq!(c.id, 2);
  assert_eq!(c.admin_level, 12);
  assert_eq!(c.distance_in_meters, Some(0));
}

#[test]
fn _16_multiple_candidates_ordered_by_distance_ascending() {
  let conn = crate::database::open_write(":memory:");
  let rows: Vec<admin_levels_row> = (0..3)
    .map(|i| make_street_row(&STREETS[i], (i + 1) as u64))
    .collect();
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});

  // every candidate is admin_level 12 (streets_for_coordinates hardcodes
  // `admin_level = 12`), so the `admin_level DESC` tier of the sort is never
  // exercised here — only the `distance ASC` tie-breaker is observable.
  let candidates = best_admin_levels(&conn, Point::new(STREETS[0].lon1, STREETS[0].lat1), None);
  assert_eq!(candidates.len(), 3);

  let distances: Vec<Option<u32>> = candidates.iter().map(|c| c.distance_in_meters).collect();
  let mut ascending = distances.clone();
  ascending.sort();
  assert_eq!(distances, ascending);
}

#[test]
fn _17_empty_linestring_candidate_is_discarded() {
  use geozero::{CoordDimensions, ToWkb};

  let conn = crate::database::open_write(":memory:");
  batch_upsert(&conn, &[make_street_row(&STREETS[0], 1)]);
  crate::index::coordinates::run(&conn, |_| {});

  // rewrite the indexed street to an empty linestring. the rtree row (built from
  // the original geometry's bbox) survives, so the candidate is still returned to
  // best_admin_levels — which must drop it silently (rej_empty). the writer's
  // ToSql rejects no-bbox geometries, so we encode the blob directly here.
  let empty: Geometry<f64> = Geometry::LineString(LineString(vec![]));
  let envelope = vec![
    STREETS[0].lon1,
    STREETS[0].lat1,
    STREETS[0].lon2,
    STREETS[0].lat2,
  ];
  let blob = empty
    .to_spatialite_wkb(CoordDimensions::default(), Some(4326), envelope)
    .expect("failed to encode empty linestring");
  conn
    .execute(SQL_UPDATE_WKB, rusqlite::params![blob, 2_i64])
    .expect("failed to update wkb");

  let candidates = best_admin_levels(&conn, Point::new(STREETS[0].lon1, STREETS[0].lat1), None);
  assert!(candidates.is_empty());
}

// a single horizontal segment A=(lon_a, lat) -> B=(lon_b, lat), used by the
// closest-point tests so the geometry of the projection is easy to reason about.
const SEG_LAT: f64 = -23.97;
const SEG_LON_A: f64 = -46.32;
const SEG_LON_B: f64 = -46.31;

fn insert_horizontal_segment(conn: &Connection) {
  let ls = LineString(vec![
    Coord {
      x: SEG_LON_A,
      y: SEG_LAT,
    },
    Coord {
      x: SEG_LON_B,
      y: SEG_LAT,
    },
  ]);
  let row = admin_levels_row {
    relation_id: None,
    way_id: Some(1),
    admin_level: 12,
    wkb: Geometry::LineString(ls).into(),
    name: "horizontal".to_string(),
    country_iso_code: None,
    post_code: None,
  };
  batch_upsert(conn, &[row]);
  crate::index::coordinates::run(conn, |_| {});
}

#[test]
fn _18_point_on_the_line_has_zero_distance() {
  let conn = crate::database::open_write(":memory:");
  insert_horizontal_segment(&conn);

  let mid_lon = (SEG_LON_A + SEG_LON_B) / 2.0;
  let candidates = best_admin_levels(&conn, Point::new(mid_lon, SEG_LAT), None);
  assert_eq!(candidates.len(), 1);
  let c = &candidates[0];

  // the query lies on the segment interior → closest point is the query itself.
  assert_eq!(c.distance_in_meters, Some(0));
  assert!((c.closest_point.x() - mid_lon).abs() < 1e-7);
  assert!((c.closest_point.y() - SEG_LAT).abs() < 1e-7);
}

#[test]
fn _19_point_perpendicular_to_segment_has_correct_distance() {
  let conn = crate::database::open_write(":memory:");
  insert_horizontal_segment(&conn);

  let mid_lon = (SEG_LON_A + SEG_LON_B) / 2.0;
  let query = Point::new(mid_lon, SEG_LAT + 0.005);
  let candidates = best_admin_levels(&conn, query, None);
  assert_eq!(candidates.len(), 1);
  let c = &candidates[0];

  // closest point is the perpendicular foot on the segment (same lon, segment lat).
  let foot = Point::new(mid_lon, SEG_LAT);
  let to_foot = query.haversine_distance(&foot);
  assert_eq!(c.distance_in_meters, Some(to_foot.round() as u32));

  // the foot is strictly nearer than either endpoint → confirms the perpendicular
  // projection was chosen rather than snapping to an endpoint.
  assert!(to_foot < query.haversine_distance(&Point::new(SEG_LON_A, SEG_LAT)));
  assert!(to_foot < query.haversine_distance(&Point::new(SEG_LON_B, SEG_LAT)));
}

#[test]
fn _20_point_beyond_endpoint_snaps_to_endpoint() {
  let conn = crate::database::open_write(":memory:");
  insert_horizontal_segment(&conn);

  // past B along the line direction → the projection clamps to endpoint B.
  let query = Point::new(SEG_LON_B + 0.005, SEG_LAT);
  let candidates = best_admin_levels(&conn, query, None);
  assert_eq!(candidates.len(), 1);
  let c = &candidates[0];

  let endpoint_b = Point::new(SEG_LON_B, SEG_LAT);
  assert!(c.closest_point.haversine_distance(&endpoint_b) < 1.0);
  assert_eq!(
    c.distance_in_meters,
    Some(query.haversine_distance(&endpoint_b).round() as u32)
  );
}
