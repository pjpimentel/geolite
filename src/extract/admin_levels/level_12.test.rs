use super::*;

fn coords(points: &[(f64, f64)]) -> Vec<Coord<f64>> {
  points.iter().map(|&(x, y)| Coord { x, y }).collect()
}

// 00.00: way sem coordenada nenhuma e descartado
#[test]
fn _00_00_way_without_coords_is_skipped() {
  assert!(process_one_way(1, &[], "Rua Augusta", None).is_none());
}

// 00.01: rua aberta vira LineString preservando a ordem das coordenadas
#[test]
fn _00_01_open_way_becomes_linestring() {
  let row = process_one_way(
    5,
    &coords(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]),
    "Rua Augusta",
    None,
  )
  .expect("deve produzir uma linha");

  match row.wkb.geometry() {
    Geometry::LineString(ls) => {
      assert_eq!(ls.0.len(), 3);
      assert!(super::super::approx_eq(ls.0[2], Coord { x: 2.0, y: 0.0 }));
    }
    other => panic!("esperado LineString, veio {other:?}"),
  }
}

// 00.02: rua com anel FECHADO continua LineString — rua nunca vira poligono,
// que e a diferenca deliberada em relacao ao nivel 10
#[test]
fn _00_02_closed_way_still_becomes_linestring() {
  let row = process_one_way(
    6,
    &coords(&[
      (0.0, 0.0),
      (1.0, 0.0),
      (1.0, 1.0),
      (0.0, 1.0),
      (0.0, 0.0),
    ]),
    "Praca do Comercio",
    None,
  )
  .expect("deve produzir uma linha");

  assert!(
    matches!(row.wkb.geometry(), Geometry::LineString(_)),
    "rua fechada nao pode virar poligono"
  );
}

// 00.03: a linha resultante e identificada por way_id e fixada no nivel 12
#[test]
fn _00_03_row_is_identified_by_way_id_at_level_12() {
  let row = process_one_way(
    77,
    &coords(&[(0.0, 0.0), (1.0, 1.0)]),
    "Rua do Ouro",
    Some("1100-060"),
  )
  .expect("deve produzir uma linha");

  pbf_fixtures::assert_admin_row(&row, 77, 12, "Rua do Ouro", Some("1100-060"));
}

// 00.04: post_code ausente e propagado como None
#[test]
fn _00_04_missing_post_code_stays_none() {
  let row = process_one_way(78, &coords(&[(0.0, 0.0), (1.0, 1.0)]), "Rua Sem CEP", None)
    .expect("deve produzir uma linha");

  assert_eq!(row.post_code, None);
}

/////////////////////////////////////////////////////////////////////////////////
// 01 — run e load_chunk ponta a ponta
/////////////////////////////////////////////////////////////////////////////////

use crate::extract::pbf_fixtures::{self, NAME_PRIORITY, stored_admin_levels, stored_geometry};

fn insert_street(conn: &Connection, way_id: u64, first_node: u64, tags: &[(&str, &str)]) {
  pbf_fixtures::insert_way_at(conn, way_id, first_node, &[(0.0, 0.0), (1.0, 0.0)], tags);
}

// 01.00: sem way com nome nao ha candidato a rua
#[test]
fn _01_00_returns_early_when_no_candidate_matches() {
  let conn = pbf_fixtures::memory_db();
  // way sem tag name e ignorado pela query de candidatos
  pbf_fixtures::insert_way_at(&conn, 10, 1, &[(0.0, 0.0)], &[("highway", "residential")]);

  let seen = pbf_fixtures::progress_events(|p| run(&conn, &[], NAME_PRIORITY, p));
  assert_eq!(seen, vec![(Some(0), 0)]);
  assert!(stored_admin_levels(&conn).is_empty());
}

// 01.01: way nomeado vira rua no nivel 12
#[test]
fn _01_01_extracts_named_way_as_a_street() {
  let conn = pbf_fixtures::memory_db();
  insert_street(
    &conn,
    10,
    1,
    &[
      ("name", "Rua Augusta"),
      ("highway", "residential"),
      ("addr:postcode", "1100-053"),
    ],
  );

  let seen = pbf_fixtures::progress_events(|p| run(&conn, &[], NAME_PRIORITY, p));

  let rows = stored_admin_levels(&conn);
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0], (Some(10), 12, "Rua Augusta".to_string()));

  let post: Option<String> = conn
    .query_row(
      "SELECT post_code FROM admin_levels WHERE way_id = 10",
      [],
      |r| r.get(0),
    )
    .expect("failed to read row");
  assert_eq!(post.as_deref(), Some("1100-053"));

  assert_eq!(seen.first().expect("evento inicial"), &(Some(1), 0));
  assert_eq!(seen.last().expect("evento final"), &(Some(1), 1));
}

// 01.02: level 12 uses EXCLUDE filters — neighbourhood, park, building and
// waterway ways stay out even when they have a name
#[test]
fn _01_02_applies_the_default_exclude_filters() {
  let conn = pbf_fixtures::memory_db();
  insert_street(&conn, 10, 1, &[("name", "Rua Boa"), ("highway", "residential")]);
  insert_street(&conn, 11, 3, &[("name", "Bairro"), ("place", "neighbourhood")]);
  insert_street(&conn, 12, 5, &[("name", "Subúrbio"), ("place", "suburb")]);
  insert_street(&conn, 13, 7, &[("name", "Parque"), ("leisure", "park")]);
  insert_street(&conn, 14, 9, &[("name", "Prédio"), ("building", "yes")]);
  insert_street(&conn, 15, 11, &[("name", "Rio"), ("waterway", "river")]);

  run(&conn, &[], NAME_PRIORITY, |_| {});

  let rows = stored_admin_levels(&conn);
  assert_eq!(rows.len(), 1, "so a rua deve sobrar");
  assert_eq!(rows[0].0, Some(10));
}

// 01.03: rua fechada continua LineString mesmo passando pelo run completo
#[test]
fn _01_03_keeps_closed_streets_as_linestring() {
  let conn = pbf_fixtures::memory_db();
  let tags = &[("name", "Praca do Comercio"), ("highway", "residential")];
  pbf_fixtures::insert_unit_square_way(&conn, 10, 1, tags);

  run(&conn, &[], NAME_PRIORITY, |_| {});

  let wkb = stored_geometry(&conn, 10);
  assert!(
    matches!(wkb.geometry(), Geometry::LineString(_)),
    "rua fechada nao pode virar poligono nem passando pelo run"
  );
}

// 01.04: override de regras substitui os filtros default de exclusao
#[test]
fn _01_04_rules_override_replaces_the_default_excludes() {
  let conn = pbf_fixtures::memory_db();
  insert_street(&conn, 11, 3, &[("name", "Bairro"), ("place", "neighbourhood")]);

  // sem filtro de exclusao nenhum, o way de bairro passa a ser aceito
  let rules = pbf_fixtures::level_rules(12, &[], &[]);
  run(&conn, &rules, NAME_PRIORITY, |_| {});

  assert_eq!(stored_admin_levels(&conn).len(), 1, "sem excludes o way de bairro deve entrar");
}

// 01.05: load_chunk agrupa coordenadas por way preservando a ordem dos refs
#[test]
fn _01_05_load_chunk_groups_coordinates_by_way_in_order() {
  let conn = pbf_fixtures::memory_db();
  pbf_fixtures::insert_way_at(
    &conn,
    10,
    1,
    &[(0.0, 0.0), (3.0, 0.0), (3.0, 4.0)],
    &[("name", "Rua Augusta")],
  );
  pbf_fixtures::insert_way_at(&conn, 11, 4, &[(9.0, 9.0)], &[("name", "Travessa")]);

  let works = load_chunk(&conn, &[10, 11], NAME_PRIORITY);

  assert_eq!(works.len(), 2);
  assert_eq!(works[0].way_id, 10);
  assert_eq!(works[0].name, "Rua Augusta");
  assert_eq!(works[0].coords.len(), 3);
  assert!((works[0].coords[2].y - 4.0).abs() < 1e-9);
  assert_eq!(works[1].way_id, 11);
  assert_eq!(works[1].coords.len(), 1);
}

// 01.06: rua ja indexada no nivel 12 nao e reprocessada
#[test]
fn _01_06_skips_ways_already_indexed_at_this_level() {
  let conn = pbf_fixtures::memory_db();
  insert_street(&conn, 10, 1, &[("name", "Rua Augusta")]);

  run(&conn, &[], NAME_PRIORITY, |_| {});

  let seen = pbf_fixtures::progress_events(|p| run(&conn, &[], NAME_PRIORITY, p));
  assert_eq!(seen, vec![(Some(0), 0)]);
}
