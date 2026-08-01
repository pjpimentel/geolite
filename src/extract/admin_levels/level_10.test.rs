use super::*;

fn coords(points: &[(f64, f64)]) -> Vec<Coord<f64>> {
  points.iter().map(|&(x, y)| Coord { x, y }).collect()
}

fn work(way_id: u64, points: &[(f64, f64)]) -> way_work {
  way_work {
    way_id,
    meta: way_meta {
      name: "Alvalade".to_string(),
      post_code: Some("1700-001".to_string()),
    },
    coords: coords(points),
    admin_level: super::super::osm_admin_level::neighborhood,
  }
}

// 00.00: way sem coordenada nenhuma e descartado
#[test]
fn _00_00_way_without_coords_is_skipped() {
  assert!(process_one_way(work(1, &[])).is_none());
}

// 00.01: anel fechado com >=4 pontos vira MultiPolygon com winding CW
// (replica o comportamento do st_buildarea do spatialite)
#[test]
fn _00_01_closed_ring_becomes_clockwise_multipolygon() {
  // quadrado em ordem CCW — deve ser invertido para CW
  let row = process_one_way(work(
    10,
    &[
      (0.0, 0.0),
      (1.0, 0.0),
      (1.0, 1.0),
      (0.0, 1.0),
      (0.0, 0.0),
    ],
  ))
  .expect("anel fechado deve produzir uma linha");

  match row.wkb.geometry() {
    Geometry::MultiPolygon(mp) => {
      assert_eq!(mp.0.len(), 1);
      assert!(
        mp.0[0].exterior().is_cw(),
        "o anel exterior deve terminar com winding CW"
      );
    }
    other => panic!("esperado MultiPolygon, veio {other:?}"),
  }
}

// 00.02: anel aberto continua LineString
#[test]
fn _00_02_open_ring_becomes_linestring() {
  let row = process_one_way(work(11, &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]))
    .expect("anel aberto ainda produz uma linha");

  assert!(
    matches!(row.wkb.geometry(), Geometry::LineString(_)),
    "anel nao fechado deve virar LineString"
  );
}

// 00.03: anel fechado com menos de 4 pontos nao satisfaz o minimo de poligono
#[test]
fn _00_03_closed_ring_with_too_few_points_is_not_a_polygon() {
  let row = process_one_way(work(12, &[(0.0, 0.0), (1.0, 0.0), (0.0, 0.0)]))
    .expect("deve produzir linha mesmo sem virar poligono");

  assert!(
    matches!(row.wkb.geometry(), Geometry::LineString(_)),
    "3 pontos nao bastam para um poligono"
  );
}

// 00.04: a linha resultante e identificada por way_id, nunca por relation_id
#[test]
fn _00_04_row_is_identified_by_way_id() {
  let row = process_one_way(work(99, &[(0.0, 0.0), (1.0, 1.0)])).expect("deve produzir uma linha");

  assert_eq!(row.way_id, Some(99));
  assert_eq!(row.relation_id, None);
  assert_eq!(row.admin_level, 10);
  assert_eq!(row.name, "Alvalade");
  assert_eq!(row.post_code.as_deref(), Some("1700-001"));
  assert_eq!(
    row.country_iso_code, None,
    "nivel 10 nunca carrega codigo de pais"
  );
}

/////////////////////////////////////////////////////////////////////////////////
// 01 — run e load_chunk ponta a ponta
/////////////////////////////////////////////////////////////////////////////////

use crate::extract::pbf_fixtures;

const NAME_PRIORITY: &[&str] = &["name"];

fn stored(conn: &Connection) -> Vec<(Option<u64>, u8, String)> {
  let mut stmt = conn
    .prepare("SELECT way_id, admin_level, name FROM admin_levels ORDER BY way_id")
    .expect("failed to prepare");
  stmt
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read row"))
    .collect()
}

// 01.00: sem way candidato o estagio encerra cedo
#[test]
fn _01_00_returns_early_when_no_candidate_matches() {
  let conn = pbf_fixtures::memory_db();
  // way com nome, mas sem tag place — nao casa com nenhum filtro de include
  pbf_fixtures::insert_node(&conn, 1, 0.0, 0.0, &[]);
  pbf_fixtures::insert_way(&conn, 10, &[1], &[("name", "Sem place")]);

  let seen = std::cell::RefCell::new(Vec::new());
  run(&conn, &[], NAME_PRIORITY, |p| {
    seen.borrow_mut().push((p.total, p.processed));
  });

  assert_eq!(seen.into_inner(), vec![(Some(0), 0)]);
  assert!(stored(&conn).is_empty());
}

// 01.01: way fechado com place=neighbourhood vira poligono no nivel 10
#[test]
fn _01_01_extracts_closed_neighbourhood_way_as_polygon() {
  let conn = pbf_fixtures::memory_db();
  pbf_fixtures::insert_closed_way(
    &conn,
    10,
    1,
    (0.0, 0.0),
    1.0,
    &[("name", "Alvalade"), ("place", "neighbourhood")],
  );

  run(&conn, &[], NAME_PRIORITY, |_| {});

  let rows = stored(&conn);
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0], (Some(10), 10, "Alvalade".to_string()));

  let wkb: crate::database::admin_levels::admin_geometry = conn
    .query_row("SELECT wkb FROM admin_levels WHERE way_id = 10", [], |r| {
      r.get(0)
    })
    .expect("failed to read geometry");
  assert!(matches!(wkb.geometry(), Geometry::MultiPolygon(_)));
}

// 01.02: place=suburb tambem entra, pelo segundo filtro de include
#[test]
fn _01_02_extracts_suburb_ways_as_well() {
  let conn = pbf_fixtures::memory_db();
  pbf_fixtures::insert_closed_way(
    &conn,
    11,
    1,
    (0.0, 0.0),
    1.0,
    &[("name", "Benfica"), ("place", "suburb")],
  );

  run(&conn, &[], NAME_PRIORITY, |_| {});

  assert_eq!(stored(&conn).len(), 1);
}

// 01.03: way aberto com place=neighbourhood vira linha, nao poligono
#[test]
fn _01_03_extracts_open_neighbourhood_way_as_linestring() {
  let conn = pbf_fixtures::memory_db();
  pbf_fixtures::insert_node(&conn, 1, 0.0, 0.0, &[]);
  pbf_fixtures::insert_node(&conn, 2, 1.0, 0.0, &[]);
  pbf_fixtures::insert_node(&conn, 3, 2.0, 1.0, &[]);
  pbf_fixtures::insert_way(
    &conn,
    10,
    &[1, 2, 3],
    &[("name", "Faixa"), ("place", "neighbourhood")],
  );

  run(&conn, &[], NAME_PRIORITY, |_| {});

  let wkb: crate::database::admin_levels::admin_geometry = conn
    .query_row("SELECT wkb FROM admin_levels WHERE way_id = 10", [], |r| {
      r.get(0)
    })
    .expect("failed to read geometry");
  assert!(matches!(wkb.geometry(), Geometry::LineString(_)));
}

// 01.04: way que casa com os DOIS filtros de include entra uma vez so
#[test]
fn _01_04_deduplicates_ways_matching_more_than_one_include_filter() {
  let conn = pbf_fixtures::memory_db();
  // place so pode ter um valor; para forcar a duplicata usamos um override
  // cujos dois filtros casam com o mesmo way
  const BOTH: &[crate::database::osm_ways::filters] = &[
    crate::database::osm_ways::filters::include_place_neighbourhood,
    crate::database::osm_ways::filters::include_place_neighbourhood,
  ];
  let rules = [super::super::extraction_rules {
    level: 10,
    include: BOTH,
    exclude: &[],
  }];
  pbf_fixtures::insert_closed_way(
    &conn,
    10,
    1,
    (0.0, 0.0),
    1.0,
    &[("name", "Alvalade"), ("place", "neighbourhood")],
  );

  run(&conn, &rules, NAME_PRIORITY, |_| {});

  assert_eq!(
    stored(&conn).len(),
    1,
    "o mesmo way nao pode ser inserido duas vezes"
  );
}

// 01.05: load_chunk agrupa as coordenadas por way, preservando a ordem dos refs
#[test]
fn _01_05_load_chunk_groups_coordinates_by_way_in_order() {
  let conn = pbf_fixtures::memory_db();
  pbf_fixtures::insert_node(&conn, 1, 0.0, 0.0, &[]);
  pbf_fixtures::insert_node(&conn, 2, 5.0, 0.0, &[]);
  pbf_fixtures::insert_node(&conn, 3, 5.0, 5.0, &[]);
  pbf_fixtures::insert_way(
    &conn,
    10,
    &[1, 2, 3],
    &[
      ("name", "Alvalade"),
      ("place", "neighbourhood"),
      ("addr:postcode", "1700-001"),
    ],
  );

  let works = load_chunk(
    &conn,
    &[10],
    super::super::osm_admin_level::neighborhood,
    NAME_PRIORITY,
  );

  assert_eq!(works.len(), 1);
  assert_eq!(works[0].way_id, 10);
  assert_eq!(works[0].meta.name, "Alvalade");
  assert_eq!(works[0].meta.post_code.as_deref(), Some("1700-001"));
  assert_eq!(works[0].coords.len(), 3);
  assert!((works[0].coords[1].x - 5.0).abs() < 1e-9);
}

// 01.06: way ja indexado no nivel 10 nao e reprocessado
#[test]
fn _01_06_skips_ways_already_indexed_at_this_level() {
  let conn = pbf_fixtures::memory_db();
  pbf_fixtures::insert_closed_way(
    &conn,
    10,
    1,
    (0.0, 0.0),
    1.0,
    &[("name", "Alvalade"), ("place", "neighbourhood")],
  );

  run(&conn, &[], NAME_PRIORITY, |_| {});
  let seen = std::cell::RefCell::new(Vec::new());
  run(&conn, &[], NAME_PRIORITY, |p| {
    seen.borrow_mut().push(p.total);
  });

  assert_eq!(
    seen.into_inner(),
    vec![Some(0)],
    "na segunda passada nao deve sobrar candidato"
  );
}
