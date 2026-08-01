use super::*;

use geo::{Coord, MultiPolygon, Polygon};

/////////////////////////////////////////////////////////////////////////////////
// auxiliares
/////////////////////////////////////////////////////////////////////////////////

fn line(points: &[(f64, f64)]) -> LineString<f64> {
  LineString(points.iter().map(|&(x, y)| Coord { x, y }).collect())
}

fn make_street(id: i64, name: &str, points: &[(f64, f64)]) -> Arc<street> {
  let geometry = MultiLineString(vec![line(points)]);
  let bbox = geometry.bounding_rect().expect("geometria deve ter bbox");
  Arc::new(street {
    id,
    name: name.to_string(),
    geometry,
    envelope: AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]),
  })
}

fn candidate(
  id: u64,
  number: &str,
  addr_street: Option<&str>,
  lon: f64,
  lat: f64,
) -> crate::database::house_numbers::candidate_row {
  crate::database::house_numbers::candidate_row {
    id,
    number: number.to_string(),
    addr_street: addr_street.map(str::to_owned),
    lon,
    lat,
  }
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
// 03 — process_tile
/////////////////////////////////////////////////////////////////////////////////

// 03.00: sem rua nenhuma no tile nao ha como casar endereco
#[test]
fn _03_00_produces_nothing_when_the_tile_has_no_streets() {
  let out = process_tile(tile_data {
    streets: Vec::new(),
    candidates: vec![candidate(1, "10", None, 0.0, 0.0)],
  });

  assert!(out.is_empty());
}

// 03.01: sem addr:street o casamento e por proximidade
#[test]
fn _03_01_matches_by_proximity_when_addr_street_is_absent() {
  let out = process_tile(tile_data {
    streets: vec![
      make_street(1, "Rua Perto", &[(0.0, 0.0), (1.0, 0.0)]),
      make_street(2, "Rua Distante", &[(0.0, 0.1), (1.0, 0.1)]),
    ],
    candidates: vec![candidate(10, "100", None, 0.5, 0.001)],
  });

  assert_eq!(out.len(), 1);
  assert_eq!(out[0].admin_level_id, 1, "deve casar com a rua mais proxima");
  assert_eq!(out[0].strategy, STRATEGY_BY_PROXIMITY);
  assert_eq!(out[0].number, "100");
  assert_eq!(out[0].node_id, 10);
}

// 03.02: addr:street batendo com o nome da rua tem precedencia sobre a proximidade
#[test]
fn _03_02_prefers_the_street_named_in_addr_street() {
  let out = process_tile(tile_data {
    streets: vec![
      make_street(1, "Rua Perto", &[(0.0, 0.0), (1.0, 0.0)]),
      make_street(2, "Rua Nomeada", &[(0.0, 0.1), (1.0, 0.1)]),
    ],
    // fisicamente mais perto da rua 1, mas o endereco cita a rua 2
    candidates: vec![candidate(10, "100", Some("Rua Nomeada"), 0.5, 0.001)],
  });

  assert_eq!(out.len(), 1);
  assert_eq!(out[0].admin_level_id, 2);
  assert_eq!(out[0].strategy, STRATEGY_BY_NAME);
}

// 03.03: name matching disregards case differences
#[test]
fn _03_03_matches_addr_street_case_insensitively() {
  let out = process_tile(tile_data {
    streets: vec![make_street(2, "Rua Nomeada", &[(0.0, 0.1), (1.0, 0.1)])],
    candidates: vec![candidate(10, "100", Some("RUA NOMEADA"), 0.5, 0.1)],
  });

  assert_eq!(out.len(), 1);
  assert_eq!(out[0].strategy, STRATEGY_BY_NAME);
}

// 03.04: entre varias ruas de mesmo nome vence a mais proxima
#[test]
fn _03_04_picks_the_nearest_among_streets_sharing_a_name() {
  let out = process_tile(tile_data {
    streets: vec![
      make_street(1, "Rua Igual", &[(0.0, 0.0), (1.0, 0.0)]),
      make_street(2, "Rua Igual", &[(0.0, 0.05), (1.0, 0.05)]),
    ],
    candidates: vec![candidate(10, "100", Some("Rua Igual"), 0.5, 0.049)],
  });

  assert_eq!(out.len(), 1);
  assert_eq!(out[0].admin_level_id, 2);
  assert_eq!(out[0].strategy, STRATEGY_BY_NAME);
}

// 03.05: addr:street que nao corresponde a nenhuma rua cai de volta na proximidade
#[test]
fn _03_05_falls_back_to_proximity_when_addr_street_matches_nothing() {
  let out = process_tile(tile_data {
    streets: vec![make_street(1, "Rua Perto", &[(0.0, 0.0), (1.0, 0.0)])],
    candidates: vec![candidate(10, "100", Some("Rua Inexistente"), 0.5, 0.001)],
  });

  assert_eq!(out.len(), 1);
  assert_eq!(out[0].admin_level_id, 1);
  assert_eq!(out[0].strategy, STRATEGY_BY_PROXIMITY);
}

// 03.06: candidato mais distante que MAX_MATCH_DEG e descartado
#[test]
fn _03_06_drops_candidates_beyond_max_match_deg() {
  let out = process_tile(tile_data {
    streets: vec![make_street(1, "Rua Distante", &[(0.0, 0.0), (1.0, 0.0)])],
    // 0.5 grau acima da rua, bem alem do corte de 0.15
    candidates: vec![candidate(10, "100", None, 0.5, 0.5)],
  });

  assert!(out.is_empty(), "MAX_MATCH_DEG deve cortar o candidato");
}

// 03.07: o ponto gravado e a projecao do endereco sobre a rua, nao o ponto original
#[test]
fn _03_07_stores_the_point_projected_onto_the_street() {
  let out = process_tile(tile_data {
    streets: vec![make_street(1, "Rua Perto", &[(0.0, 0.0), (10.0, 0.0)])],
    candidates: vec![candidate(10, "100", None, 5.0, 0.01)],
  });

  assert_eq!(out.len(), 1);
  match out[0].wkb.geometry() {
    Geometry::Point(p) => {
      assert!((p.x() - 5.0).abs() < 1e-9);
      assert!((p.y() - 0.0).abs() < 1e-9, "deve estar projetado sobre a rua");
    }
    other => panic!("esperado Point, veio {other:?}"),
  }
}

/////////////////////////////////////////////////////////////////////////////////
// 04 — run ponta a ponta
/////////////////////////////////////////////////////////////////////////////////

fn setup_db() -> rusqlite::Connection {
  crate::database::open_write(":memory:")
}

fn insert_street(conn: &rusqlite::Connection, id: u64, name: &str, points: &[(f64, f64)]) {
  crate::database::admin_levels::batch_upsert(
    conn,
    &[crate::database::admin_levels::admin_levels {
      relation_id: None,
      way_id: Some(id),
      admin_level: 12,
      name: name.to_string(),
      country_iso_code: None,
      post_code: None,
      wkb: Geometry::LineString(line(points)).into(),
    }],
  );
}

fn insert_house_node(conn: &rusqlite::Connection, id: u64, lon: f64, lat: f64, tags: &[(&str, &str)]) {
  let node = crate::extract::osm_data::osm_nodes::osm_node {
    id: id as i64,
    lat,
    lon,
    tags: tags
      .iter()
      .map(|&(k, v)| (k.to_string(), v.to_string()))
      .collect(),
  };
  let mut payload = Vec::new();
  crate::extract::osm_data::jsonb_encode::encoder::new().encode_osm_node(&mut payload, &node);
  crate::database::osm_nodes::insert_rows(
    conn,
    &[crate::database::osm_nodes::osm_node_row {
      id,
      osm_pbf_chunk_id: 0,
      payload,
    }],
  );
}

fn preset() -> crate::presets::extract_house_numbers_preset {
  crate::presets::resolve(None).extract_house_numbers
}

fn stored(conn: &rusqlite::Connection) -> Vec<(i64, i64, String, i64)> {
  let mut stmt = conn
    .prepare("SELECT node_id, admin_level_id, number, strategy FROM house_numbers ORDER BY node_id")
    .expect("failed to prepare");
  stmt
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read row"))
    .collect()
}

// 04.00: sem candidato nenhum o run encerra cedo reportando total zero
#[test]
fn _04_00_returns_early_when_there_are_no_candidates() {
  let conn = setup_db();
  insert_street(&conn, 1, "Rua Vazia", &[(0.0, 0.0), (1.0, 0.0)]);

  let seen = std::cell::RefCell::new(Vec::new());
  run(&conn, preset(), |p| {
    seen.borrow_mut().push((p.total, p.processed));
  });

  assert_eq!(seen.into_inner(), vec![(0, 0)]);
  assert!(stored(&conn).is_empty());
}

// 04.01: fluxo completo — casa por proximidade e grava a linha
#[test]
fn _04_01_matches_and_persists_house_numbers_by_proximity() {
  let conn = setup_db();
  insert_street(&conn, 1, "Rua Augusta", &[(0.0, 0.0), (1.0, 0.0)]);
  insert_house_node(&conn, 10, 0.5, 0.001, &[("addr:housenumber", "100")]);

  let seen = std::cell::RefCell::new(Vec::new());
  run(&conn, preset(), |p| {
    seen.borrow_mut().push((p.total, p.processed));
  });

  let rows = stored(&conn);
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].0, 10, "node_id");
  assert_eq!(rows[0].2, "100", "number");
  assert_eq!(rows[0].3, STRATEGY_BY_PROXIMITY as i64);

  let seen = seen.into_inner();
  assert_eq!(seen.first().expect("evento inicial"), &(1, 0));
  assert_eq!(seen.last().expect("evento final"), &(1, 1));
}

// 04.02: addr:street presente muda a estrategia para casamento por nome
#[test]
fn _04_02_matches_by_name_when_addr_street_is_present() {
  let conn = setup_db();
  insert_street(&conn, 1, "Rua Perto", &[(0.0, 0.0), (1.0, 0.0)]);
  insert_street(&conn, 2, "Rua Nomeada", &[(0.0, 0.1), (1.0, 0.1)]);
  insert_house_node(
    &conn,
    10,
    0.5,
    0.001,
    &[("addr:housenumber", "100"), ("addr:street", "Rua Nomeada")],
  );

  run(&conn, preset(), |_| {});

  let rows = stored(&conn);
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].3, STRATEGY_BY_NAME as i64);
}

// 04.03: rua no tile vizinho ainda e considerada, gracas a expansao de +-1 tile
#[test]
fn _04_03_considers_streets_from_neighbouring_tiles() {
  let conn = setup_db();
  // TILE_SIZE e 2.0: a rua fica no tile x=0 e o endereco no tile x=1
  insert_street(&conn, 1, "Rua da Fronteira", &[(1.9, 0.0), (2.1, 0.0)]);
  insert_house_node(&conn, 10, 2.05, 0.001, &[("addr:housenumber", "100")]);

  run(&conn, preset(), |_| {});

  let rows = stored(&conn);
  assert_eq!(
    rows.len(),
    1,
    "a rua do tile vizinho deveria entrar na busca"
  );
  assert_eq!(rows[0].1, stored(&conn)[0].1);
}

// 04.04: candidates spread across several tiles are all processed
#[test]
fn _04_04_processes_candidates_spread_across_tiles() {
  let conn = setup_db();
  insert_street(&conn, 1, "Rua Um", &[(0.0, 0.0), (1.0, 0.0)]);
  insert_street(&conn, 2, "Rua Dois", &[(10.0, 10.0), (11.0, 10.0)]);
  insert_house_node(&conn, 10, 0.5, 0.001, &[("addr:housenumber", "100")]);
  insert_house_node(&conn, 20, 10.5, 10.001, &[("addr:housenumber", "200")]);

  run(&conn, preset(), |_| {});

  let rows = stored(&conn);
  assert_eq!(rows.len(), 2);
  assert_eq!(rows[0].2, "100");
  assert_eq!(rows[1].2, "200");
}

// 04.05: endereco distante de qualquer rua nao vira linha
#[test]
fn _04_05_skips_candidates_with_no_street_within_range() {
  let conn = setup_db();
  insert_street(&conn, 1, "Rua Distante", &[(0.0, 0.0), (1.0, 0.0)]);
  insert_house_node(&conn, 10, 50.0, 50.0, &[("addr:housenumber", "100")]);

  run(&conn, preset(), |_| {});

  assert!(stored(&conn).is_empty());
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

// 04.06: rua cuja geometria nao e linha nem area e ignorada no carregamento
#[test]
fn _04_06_skips_streets_whose_geometry_is_not_linear() {
  let conn = setup_db();
  // rua gravada como ponto: geometry_to_multilinestring devolve None
  crate::database::admin_levels::batch_upsert(
    &conn,
    &[crate::database::admin_levels::admin_levels {
      relation_id: None,
      way_id: Some(1),
      admin_level: 12,
      name: "Rua Pontual".to_string(),
      country_iso_code: None,
      post_code: None,
      wkb: Geometry::Point(Point::new(0.0, 0.0)).into(),
    }],
  );
  insert_house_node(&conn, 10, 0.001, 0.001, &[("addr:housenumber", "100")]);

  run(&conn, preset(), |_| {});

  assert!(
    stored(&conn).is_empty(),
    "rua sem geometria linear nao pode casar endereco"
  );
}
