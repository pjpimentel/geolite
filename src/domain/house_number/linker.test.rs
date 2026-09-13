use super::{closest_point_on_geometry, geometry_to_multilinestring, indexed_street, link_tile, street, tile_data};
use crate::domain::house_number::house_number_link;
use crate::domain::house_number::repository::candidate_row;
use crate::domain::house_number::strategy::link_strategy;
use geo::{Geometry, MultiLineString, Point};
use rstar::{AABB, PointDistance, RTreeObject};
use std::sync::Arc;

use crate::domain::house_number::fixtures::line;
use crate::domain::house_number::house_number;
use geo::{MultiPolygon, Polygon};

/////////////////////////////////////////////////////////////////////////////////
// auxiliares
/////////////////////////////////////////////////////////////////////////////////

fn make_street(id: i64, name: &str, points: &[(f64, f64)]) -> Arc<street> {
  Arc::new(
    street::from_geometry(id, name.to_string(), &Geometry::LineString(line(points)))
      .expect("geometria deve ter bbox"),
  )
}

fn candidate(id: u64, number: &str, addr_street: Option<&str>, lon: f64, lat: f64) -> candidate_row {
  candidate_row {
    id,
    number: house_number::from_stored(number),
    addr_street: addr_street.map(str::to_owned),
    lon,
    lat,
  }
}

// duas ruas paralelas — a primeira em y=0, a segunda em y=second_y — e um unico
// candidato em (0.5, candidate_y)
fn two_parallel_streets(
  names: (&str, &str),
  second_y: f64,
  addr_street: Option<&str>,
  candidate_y: f64,
) -> Vec<house_number_link> {
  link_tile(tile_data {
    streets: vec![
      make_street(1, names.0, &[(0.0, 0.0), (1.0, 0.0)]),
      make_street(2, names.1, &[(0.0, second_y), (1.0, second_y)]),
    ],
    candidates: vec![candidate(10, "100", addr_street, 0.5, candidate_y)],
  })
}

/////////////////////////////////////////////////////////////////////////////////
// 00 — geometry_to_multilinestring
/////////////////////////////////////////////////////////////////////////////////

// 00.00: LineString vira uma multilinha de um elemento
#[test]
fn _00_00_converts_linestring() {
  let geom = Geometry::LineString(line(&[(0.0, 0.0), (1.0, 1.0)]));
  let mls = geometry_to_multilinestring(&geom).expect("deve converter");
  assert_eq!(mls.0.len(), 1);
}

// 00.01: MultiLineString e preservada como esta
#[test]
fn _00_01_preserves_multilinestring() {
  let geom = Geometry::MultiLineString(MultiLineString(vec![
    line(&[(0.0, 0.0), (1.0, 0.0)]),
    line(&[(2.0, 2.0), (3.0, 3.0)]),
  ]));
  let mls = geometry_to_multilinestring(&geom).expect("deve converter");
  assert_eq!(mls.0.len(), 2);
}

// 00.02: Polygon contribui com o anel exterior mais cada anel interior
#[test]
fn _00_02_converts_polygon_exterior_and_interiors() {
  let exterior = line(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.0)]);
  let hole = line(&[(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 2.0), (1.0, 1.0)]);
  let geom = Geometry::Polygon(Polygon::new(exterior, vec![hole]));

  let mls = geometry_to_multilinestring(&geom).expect("deve converter");
  assert_eq!(mls.0.len(), 2, "exterior + 1 buraco");
}

// 00.03: MultiPolygon soma os aneis de todos os poligonos
#[test]
fn _00_03_converts_multipolygon() {
  let a = Polygon::new(
    line(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.0)]),
    vec![line(&[
      (0.2, 0.2),
      (0.4, 0.2),
      (0.4, 0.4),
      (0.2, 0.2),
    ])],
  );
  let b = Polygon::new(
    line(&[(5.0, 5.0), (6.0, 5.0), (6.0, 6.0), (5.0, 5.0)]),
    vec![],
  );
  let geom = Geometry::MultiPolygon(MultiPolygon(vec![a, b]));

  let mls = geometry_to_multilinestring(&geom).expect("deve converter");
  assert_eq!(mls.0.len(), 3, "2 exteriores + 1 buraco");
}

// 00.04: geometrias que nao sao linhas nem areas nao tem como virar multilinha
#[test]
fn _00_04_rejects_unsupported_geometry_kinds() {
  assert!(geometry_to_multilinestring(&Geometry::Point(Point::new(1.0, 2.0))).is_none());
}

// 00.05: multilinha cujas linhas estao todas vazias nao serve para casar endereco
#[test]
fn _00_05_rejects_geometry_whose_lines_are_all_empty() {
  let geom = Geometry::MultiLineString(MultiLineString(vec![line(&[]), line(&[])]));
  assert!(geometry_to_multilinestring(&geom).is_none());
}

/////////////////////////////////////////////////////////////////////////////////
// 01 — closest_point_on_geometry
/////////////////////////////////////////////////////////////////////////////////

// 01.00: projects the point onto the closest segment among several
#[test]
fn _01_00_returns_the_closest_point_across_segments() {
  let geom = MultiLineString(vec![
    line(&[(0.0, 0.0), (10.0, 0.0)]),
    line(&[(0.0, 100.0), (10.0, 100.0)]),
  ]);

  let cp = closest_point_on_geometry(&geom, &Point::new(5.0, 1.0)).expect("deve projetar");
  assert!((cp.x() - 5.0).abs() < 1e-9);
  assert!(
    (cp.y() - 0.0).abs() < 1e-9,
    "deve escolher o segmento de baixo, nao o de y=100"
  );
}

// 01.01: linhas vazias sao ignoradas em vez de derrubar a projecao
#[test]
fn _01_01_skips_empty_linestrings() {
  let geom = MultiLineString(vec![line(&[]), line(&[(0.0, 0.0), (10.0, 0.0)])]);

  let cp = closest_point_on_geometry(&geom, &Point::new(5.0, 1.0)).expect("deve projetar");
  assert!((cp.x() - 5.0).abs() < 1e-9);
}

// 01.02: sem nenhuma linha utilizavel nao ha ponto para devolver
#[test]
fn _01_02_returns_none_when_every_linestring_is_empty() {
  let geom = MultiLineString(vec![line(&[]), line(&[])]);
  assert!(closest_point_on_geometry(&geom, &Point::new(0.0, 0.0)).is_none());
}

// 01.03: linha com um unico ponto nao define segmento, entao nao ha projecao
#[test]
fn _01_03_skips_linestrings_without_a_segment() {
  let geom = MultiLineString(vec![line(&[(5.0, 5.0)])]);
  assert!(
    closest_point_on_geometry(&geom, &Point::new(0.0, 0.0)).is_none(),
    "linha de um ponto so nao produz projecao"
  );
}

/////////////////////////////////////////////////////////////////////////////////
// 02 — indexed_street (rstar)
/////////////////////////////////////////////////////////////////////////////////

// 02.00: o envelope indexado e o mesmo bbox calculado para a rua
#[test]
fn _02_00_exposes_the_street_envelope() {
  let s = make_street(1, "Rua A", &[(0.0, 0.0), (2.0, 3.0)]);
  let indexed = indexed_street { data: s };

  assert_eq!(
    indexed.envelope(),
    AABB::from_corners([0.0, 0.0], [2.0, 3.0])
  );
}

// 02.01: distance_2 devolve o QUADRADO da distancia euclidiana ate a geometria
#[test]
fn _02_01_returns_squared_distance_to_the_geometry() {
  let s = make_street(1, "Rua A", &[(0.0, 0.0), (10.0, 0.0)]);
  let indexed = indexed_street { data: s };

  // ponto a 3 unidades acima do segmento → distancia^2 = 9
  assert!((indexed.distance_2(&[5.0, 3.0]) - 9.0).abs() < 1e-9);
}

/////////////////////////////////////////////////////////////////////////////////
// 03 — link_tile
/////////////////////////////////////////////////////////////////////////////////

// 03.00: sem rua nenhuma no tile nao ha como casar endereco
#[test]
fn _03_00_produces_nothing_when_the_tile_has_no_streets() {
  let out = link_tile(tile_data {
    streets: Vec::new(),
    candidates: vec![candidate(1, "10", None, 0.0, 0.0)],
  });

  assert!(out.is_empty());
}

// 03.01: sem addr:street o casamento e por proximidade
#[test]
fn _03_01_matches_by_proximity_when_addr_street_is_absent() {
  let out = two_parallel_streets(("Rua Perto", "Rua Distante"), 0.1, None, 0.001);

  assert_eq!(out.len(), 1);
  assert_eq!(out[0].street_id.raw(), 1, "deve casar com a rua mais proxima");
  assert_eq!(out[0].strategy, link_strategy::by_proximity);
  assert_eq!(out[0].number.stored_form(), "100");
  assert_eq!(out[0].node_id, 10);
}

// 03.02: addr:street batendo com o nome da rua tem precedencia sobre a proximidade
#[test]
fn _03_02_prefers_the_street_named_in_addr_street() {
  // o candidato fica fisicamente mais perto da rua 1, mas cita a rua 2
  let out = two_parallel_streets(
    ("Rua Perto", "Rua Nomeada"),
    0.1,
    Some("Rua Nomeada"),
    0.001,
  );

  assert_eq!(out.len(), 1);
  assert_eq!(out[0].street_id.raw(), 2);
  assert_eq!(out[0].strategy, link_strategy::by_name);
}

// 03.03: name matching disregards case differences
#[test]
fn _03_03_matches_addr_street_case_insensitively() {
  let out = link_tile(tile_data {
    streets: vec![make_street(2, "Rua Nomeada", &[(0.0, 0.1), (1.0, 0.1)])],
    candidates: vec![candidate(10, "100", Some("RUA NOMEADA"), 0.5, 0.1)],
  });

  assert_eq!(out.len(), 1);
  assert_eq!(out[0].strategy, link_strategy::by_name);
}

// 03.04: entre varias ruas de mesmo nome vence a mais proxima
#[test]
fn _03_04_picks_the_nearest_among_streets_sharing_a_name() {
  let out = two_parallel_streets(("Rua Igual", "Rua Igual"), 0.05, Some("Rua Igual"), 0.049);

  assert_eq!(out.len(), 1);
  assert_eq!(out[0].street_id.raw(), 2);
  assert_eq!(out[0].strategy, link_strategy::by_name);
}

// 03.05: addr:street que nao corresponde a nenhuma rua cai de volta na proximidade
#[test]
fn _03_05_falls_back_to_proximity_when_addr_street_matches_nothing() {
  let out = link_tile(tile_data {
    streets: vec![make_street(1, "Rua Perto", &[(0.0, 0.0), (1.0, 0.0)])],
    candidates: vec![candidate(10, "100", Some("Rua Inexistente"), 0.5, 0.001)],
  });

  assert_eq!(out.len(), 1);
  assert_eq!(out[0].street_id.raw(), 1);
  assert_eq!(out[0].strategy, link_strategy::by_proximity);
}

// 03.06: candidato mais distante que MAX_MATCH_DEG e descartado
#[test]
fn _03_06_drops_candidates_beyond_max_match_deg() {
  let out = link_tile(tile_data {
    streets: vec![make_street(1, "Rua Distante", &[(0.0, 0.0), (1.0, 0.0)])],
    // 0.5 grau acima da rua, bem alem do corte de 0.15
    candidates: vec![candidate(10, "100", None, 0.5, 0.5)],
  });

  assert!(out.is_empty(), "MAX_MATCH_DEG deve cortar o candidato");
}

// 03.07: o ponto gravado e a projecao do endereco sobre a rua, nao o ponto original
#[test]
fn _03_07_stores_the_point_projected_onto_the_street() {
  let out = link_tile(tile_data {
    streets: vec![make_street(1, "Rua Perto", &[(0.0, 0.0), (10.0, 0.0)])],
    candidates: vec![candidate(10, "100", None, 5.0, 0.01)],
  });

  assert_eq!(out.len(), 1);
  let p = out[0].point;
  assert!((p.x() - 5.0).abs() < 1e-9);
  assert!((p.y() - 0.0).abs() < 1e-9, "deve estar projetado sobre a rua");
}
