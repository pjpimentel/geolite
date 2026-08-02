use super::*;

use crate::extract::pbf_fixtures;

fn ls(points: &[(f64, f64)]) -> LineString<f64> {
  LineString(points.iter().map(|&(x, y)| Coord { x, y }).collect())
}

fn meta() -> rel_meta {
  rel_meta {
    name: "Lisboa".to_string(),
    country_iso_code: Some("PT".to_string()),
    post_code: Some("1000-001".to_string()),
  }
}

// 00.00: nivel sem override e sem default retorna listas vazias
#[test]
fn _00_00_level_without_rules_resolves_to_empty_lists() {
  let (include, exclude) = resolve_rules(8, &[]);
  assert!(include.is_empty());
  assert!(exclude.is_empty());
}

// 00.01: nivel 10 sem override cai no default_include (place=neighbourhood + place=suburb)
#[test]
fn _00_01_level_10_resolves_to_default_include() {
  let (include, exclude) = resolve_rules(10, &[]);
  assert_eq!(include.len(), 2);
  assert!(matches!(
    include[0],
    crate::database::osm_ways::filters::include_place_neighbourhood
  ));
  assert!(matches!(
    include[1],
    crate::database::osm_ways::filters::include_place_suburb
  ));
  assert!(exclude.is_empty());
}

// 00.02: nivel 12 sem override cai no default_exclude (5 filtros de exclusao)
#[test]
fn _00_02_level_12_resolves_to_default_exclude() {
  let (include, exclude) = resolve_rules(12, &[]);
  assert!(include.is_empty());
  assert_eq!(exclude.len(), 5);
  assert!(matches!(
    exclude[0],
    crate::database::osm_ways::filters::exclude_place_neighbourhood
  ));
  assert!(matches!(
    exclude[4],
    crate::database::osm_ways::filters::exclude_waterway
  ));
}

// 00.03: override do mesmo nivel tem precedencia sobre o default
#[test]
fn _00_03_override_takes_precedence_over_default() {
  const INCLUDE: &[crate::database::osm_ways::filters] =
    &[crate::database::osm_ways::filters::include_highway_primary];
  const EXCLUDE: &[crate::database::osm_ways::filters] =
    &[crate::database::osm_ways::filters::exclude_building];
  let overrides = [extraction_rules {
    level: 12,
    include: INCLUDE,
    exclude: EXCLUDE,
  }];

  let (include, exclude) = resolve_rules(12, &overrides);
  assert_eq!(include.len(), 1);
  assert!(matches!(
    include[0],
    crate::database::osm_ways::filters::include_highway_primary
  ));
  assert_eq!(exclude.len(), 1);
}

// 00.04: override de outro nivel nao afeta o nivel consultado
#[test]
fn _00_04_override_for_another_level_is_ignored() {
  const INCLUDE: &[crate::database::osm_ways::filters] =
    &[crate::database::osm_ways::filters::include_highway_primary];
  let overrides = [extraction_rules {
    level: 4,
    include: INCLUDE,
    exclude: &[],
  }];

  let (include, exclude) = resolve_rules(12, &overrides);
  assert!(include.is_empty(), "nivel 12 nao deve herdar override do 4");
  assert_eq!(exclude.len(), 5, "nivel 12 mantem o default_exclude");
}

// 01.00: todos os 13 valores mapeados de u8 para osm_admin_level
#[test]
fn _01_00_maps_every_valid_level_value() {
  for level in [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 14, 30] {
    let parsed = osm_admin_level::try_from(level)
      .map(|l| l as u8)
      .unwrap_or_else(|_| panic!("level {level} deveria ser valido"));
    assert_eq!(parsed, level);
  }
}

// 01.01: valores fora da tabela retornam Err carregando o valor original
#[test]
fn _01_01_unknown_level_value_returns_error_with_input() {
  for level in [0u8, 11, 13, 15, 29, 31, 255] {
    assert!(
      matches!(osm_admin_level::try_from(level), Err(v) if v == level),
      "level {level} deveria ser invalido"
    );
  }
}

// 02.00: coordenadas identicas sao aproximadamente iguais
#[test]
fn _02_00_identical_coords_are_equal() {
  assert!(approx_eq(
    Coord { x: 1.5, y: -2.5 },
    Coord { x: 1.5, y: -2.5 }
  ));
}

// 02.01: diferenca abaixo da tolerancia de 1e-9 ainda conta como igual
#[test]
fn _02_01_difference_below_tolerance_is_equal() {
  assert!(approx_eq(
    Coord { x: 1.0, y: 1.0 },
    Coord {
      x: 1.0 + 1e-12,
      y: 1.0 - 1e-12
    }
  ));
}

// 02.02: divergencia acima da tolerancia em x ou em y quebra a igualdade
#[test]
fn _02_02_difference_above_tolerance_is_not_equal() {
  assert!(!approx_eq(Coord { x: 1.0, y: 1.0 }, Coord { x: 1.1, y: 1.0 }));
  assert!(!approx_eq(Coord { x: 1.0, y: 1.0 }, Coord { x: 1.0, y: 1.1 }));
}

// 03.00: lista vazia de ways nao produz anel nenhum
#[test]
fn _03_00_no_ways_produces_no_rings() {
  assert!(assemble_rings(&[]).is_empty());
}

// 03.01: way unico vira um anel com as mesmas coordenadas
#[test]
fn _03_01_single_way_becomes_a_single_ring() {
  let rings = assemble_rings(&[ls(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)])]);
  assert_eq!(rings.len(), 1);
  assert_eq!(rings[0].0.len(), 3);
}

// 03.02: fim do way A encosta no inicio do way B → juncao direta, sem inverter B
#[test]
fn _03_02_joins_ways_head_to_tail() {
  let rings = assemble_rings(&[
    ls(&[(0.0, 0.0), (1.0, 0.0)]),
    ls(&[(1.0, 0.0), (1.0, 1.0)]),
  ]);
  assert_eq!(rings.len(), 1);
  assert_eq!(rings[0].0.len(), 3);
  assert!(approx_eq(rings[0].0[2], Coord { x: 1.0, y: 1.0 }));
}

// 03.03: fim do way A encosta no FIM do way B → B precisa ser invertido antes de juntar
#[test]
fn _03_03_reverses_way_when_joining_tail_to_tail() {
  let rings = assemble_rings(&[
    ls(&[(0.0, 0.0), (1.0, 0.0)]),
    ls(&[(1.0, 1.0), (1.0, 0.0)]),
  ]);
  assert_eq!(rings.len(), 1);
  assert_eq!(rings[0].0.len(), 3);
  assert!(
    approx_eq(rings[0].0[2], Coord { x: 1.0, y: 1.0 }),
    "o way invertido deve terminar no seu ponto inicial original"
  );
}

// 03.04: quatro segmentos encadeados fecham um quadrado (primeiro ponto == ultimo)
#[test]
fn _03_04_chained_segments_close_into_a_ring() {
  let rings = assemble_rings(&[
    ls(&[(0.0, 0.0), (1.0, 0.0)]),
    ls(&[(1.0, 0.0), (1.0, 1.0)]),
    ls(&[(1.0, 1.0), (0.0, 1.0)]),
    ls(&[(0.0, 1.0), (0.0, 0.0)]),
  ]);
  assert_eq!(rings.len(), 1);
  assert_eq!(rings[0].0.len(), 5);
  assert!(approx_eq(rings[0].0[0], *rings[0].0.last().unwrap()));
}

// 03.05: segmentos que nao se tocam viram aneis separados
#[test]
fn _03_05_disjoint_ways_become_separate_rings() {
  let rings = assemble_rings(&[
    ls(&[(0.0, 0.0), (1.0, 0.0)]),
    ls(&[(10.0, 10.0), (11.0, 10.0)]),
  ]);
  assert_eq!(rings.len(), 2);
}

// 04.00: relation cujas ways estao todas vazias e descartada
#[test]
fn _04_00_relation_with_only_empty_ways_is_skipped() {
  let result = process_one_relation(1, &meta(), &[ls(&[]), ls(&[])], osm_admin_level::city);
  assert!(result.is_none());
}

// 04.01: anel fechado com >=4 pontos vira MultiPolygon com winding CW
// (replica o comportamento do st_buildarea do spatialite)
#[test]
fn _04_01_closed_ring_becomes_clockwise_multipolygon() {
  // quadrado em ordem CCW — process_one_relation deve inverter para CW
  let ways = [ls(&[
    (0.0, 0.0),
    (1.0, 0.0),
    (1.0, 1.0),
    (0.0, 1.0),
    (0.0, 0.0),
  ])];
  let row = process_one_relation(7, &meta(), &ways, osm_admin_level::city)
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

// 04.02: anel aberto nao vira poligono — cai no MultiLineString
#[test]
fn _04_02_open_ring_becomes_multilinestring() {
  let ways = [ls(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)])];
  let row = process_one_relation(8, &meta(), &ways, osm_admin_level::city)
    .expect("anel aberto ainda produz uma linha");

  assert!(
    matches!(row.wkb.geometry(), Geometry::MultiLineString(_)),
    "anel nao fechado deve virar MultiLineString"
  );
}

// 04.03: anel fechado com menos de 4 pontos nao satisfaz o minimo de poligono
#[test]
fn _04_03_closed_ring_with_too_few_points_is_not_a_polygon() {
  let ways = [ls(&[(0.0, 0.0), (1.0, 0.0), (0.0, 0.0)])];
  let row = process_one_relation(9, &meta(), &ways, osm_admin_level::city)
    .expect("deve produzir linha mesmo sem virar poligono");

  assert!(
    matches!(row.wkb.geometry(), Geometry::MultiLineString(_)),
    "3 pontos nao bastam para um poligono"
  );
}

// 04.04: metadados e admin_level sao propagados para a linha resultante
#[test]
fn _04_04_propagates_metadata_and_admin_level() {
  let ways = [ls(&[(0.0, 0.0), (1.0, 0.0)])];
  let row = process_one_relation(42, &meta(), &ways, osm_admin_level::municipality)
    .expect("deve produzir uma linha");

  assert_eq!(row.relation_id, Some(42));
  assert_eq!(row.way_id, None);
  assert_eq!(row.admin_level, 7);
  assert_eq!(row.name, "Lisboa");
  assert_eq!(row.country_iso_code.as_deref(), Some("PT"));
  assert_eq!(row.post_code.as_deref(), Some("1000-001"));
}

/////////////////////////////////////////////////////////////////////////////////
// 05 — run_with_ids e load_and_send ponta a ponta
/////////////////////////////////////////////////////////////////////////////////

// relation 500 formada por 2 ways que juntos fecham um quadrado
fn setup_square_relation(conn: &rusqlite::Connection) {
  for (id, (x, y)) in [(1u64, (0.0, 0.0)), (2, (1.0, 0.0)), (3, (1.0, 1.0)), (4, (0.0, 1.0))] {
    pbf_fixtures::insert_node(conn, id, x, y, &[]);
  }
  // way 10: canto 1 -> 2 -> 3 ; way 11: canto 3 -> 4 -> 1 (fecha o anel)
  pbf_fixtures::insert_way(conn, 10, &[1, 2, 3], &[("name", "Trecho A")]);
  pbf_fixtures::insert_way(conn, 11, &[3, 4, 1], &[("name", "Trecho B")]);
  pbf_fixtures::insert_relation(
    conn,
    500,
    &[(1, 10, "outer"), (1, 11, "outer")],
    &[
      ("name", "Lisboa"),
      ("ISO3166-1", "pt"),
      ("addr:postcode", "1000-001"),
    ],
  );
}

fn stored_levels(conn: &rusqlite::Connection) -> Vec<(Option<u64>, u8, String)> {
  let mut stmt = conn
    .prepare("SELECT relation_id, admin_level, name FROM admin_levels ORDER BY id")
    .expect("failed to prepare");
  stmt
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read row"))
    .collect()
}

const NAME_PRIORITY: &[&str] = &["name"];

// 05.00: sem ids a processar o estagio encerra cedo, reportando total zero
#[test]
fn _05_00_returns_early_when_the_id_list_is_empty() {
  let conn = pbf_fixtures::memory_db();

  let seen = std::cell::RefCell::new(Vec::new());
  run_with_ids(
    &conn,
    Vec::new(),
    osm_admin_level::city,
    1,
    NAME_PRIORITY,
    |p| seen.borrow_mut().push((p.total, p.processed)),
  );

  assert_eq!(seen.into_inner(), vec![(Some(0), 0)]);
  assert!(stored_levels(&conn).is_empty());
}

// 05.01: relation com ways que fecham um anel vira MultiPolygon persistido,
// carregando nome, codigo de pais e codigo postal vindos das tags
#[test]
fn _05_01_upserts_a_multipolygon_for_a_closed_relation() {
  let conn = pbf_fixtures::memory_db();
  setup_square_relation(&conn);

  let seen = std::cell::RefCell::new(Vec::new());
  run_with_ids(
    &conn,
    vec![500],
    osm_admin_level::city,
    1,
    NAME_PRIORITY,
    |p| seen.borrow_mut().push((p.total, p.processed)),
  );

  let rows = stored_levels(&conn);
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0], (Some(500), 8, "Lisboa".to_string()));

  let (iso, post): (Option<String>, Option<String>) = conn
    .query_row(
      "SELECT country_iso_code, post_code FROM admin_levels WHERE relation_id = 500",
      [],
      |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .expect("failed to read row");
  assert_eq!(iso.as_deref(), Some("PT"));
  assert_eq!(post.as_deref(), Some("1000-001"));

  let seen = seen.into_inner();
  assert_eq!(seen.first().expect("evento inicial"), &(Some(1), 0));
  assert_eq!(seen.last().expect("evento final"), &(Some(1), 1));
}

// 05.02: relation cujos ways nao fecham anel vira MultiLineString
#[test]
fn _05_02_falls_back_to_multilinestring_for_open_relations() {
  let conn = pbf_fixtures::memory_db();
  pbf_fixtures::insert_node(&conn, 1, 0.0, 0.0, &[]);
  pbf_fixtures::insert_node(&conn, 2, 1.0, 0.0, &[]);
  pbf_fixtures::insert_node(&conn, 3, 2.0, 1.0, &[]);
  pbf_fixtures::insert_way(&conn, 10, &[1, 2, 3], &[("name", "Trecho aberto")]);
  pbf_fixtures::insert_relation(&conn, 500, &[(1, 10, "outer")], &[("name", "Aberta")]);

  run_with_ids(&conn, vec![500], osm_admin_level::city, 1, NAME_PRIORITY, |_| {});

  let wkb: crate::database::admin_levels::admin_geometry = conn
    .query_row(
      "SELECT wkb FROM admin_levels WHERE relation_id = 500",
      [],
      |r| r.get(0),
    )
    .expect("failed to read geometry");

  assert!(
    matches!(wkb.geometry(), Geometry::MultiLineString(_)),
    "anel aberto deve virar MultiLineString"
  );
}

// 05.03: membros que nao sao way sao ignorados pela query de coordenadas
#[test]
fn _05_03_ignores_relation_members_that_are_not_ways() {
  let conn = pbf_fixtures::memory_db();
  setup_square_relation(&conn);
  pbf_fixtures::insert_relation(
    &conn,
    501,
    &[(0, 1, "admin_centre"), (2, 500, "subarea")],
    &[("name", "So membros nao-way")],
  );

  run_with_ids(&conn, vec![501], osm_admin_level::city, 1, NAME_PRIORITY, |_| {});

  assert!(
    stored_levels(&conn).is_empty(),
    "sem way nenhum nao ha geometria a montar"
  );
}

// 05.04: several relations are processed and progress accumulates
#[test]
fn _05_04_processes_several_relations_and_accumulates_progress() {
  let conn = pbf_fixtures::memory_db();
  setup_square_relation(&conn);
  pbf_fixtures::insert_node(&conn, 20, 5.0, 5.0, &[]);
  pbf_fixtures::insert_node(&conn, 21, 6.0, 5.0, &[]);
  pbf_fixtures::insert_way(&conn, 30, &[20, 21], &[("name", "Outro trecho")]);
  pbf_fixtures::insert_relation(&conn, 501, &[(1, 30, "outer")], &[("name", "Porto")]);

  let seen = std::cell::RefCell::new(Vec::new());
  run_with_ids(
    &conn,
    vec![500, 501],
    osm_admin_level::city,
    1,
    NAME_PRIORITY,
    |p| seen.borrow_mut().push(p.processed),
  );

  assert_eq!(stored_levels(&conn).len(), 2);
  assert_eq!(seen.into_inner().last().copied(), Some(2));
}

// 05.05: com mais de uma thread o resultado continua o mesmo
#[test]
fn _05_05_produces_the_same_rows_with_multiple_threads() {
  let conn = pbf_fixtures::memory_db();
  setup_square_relation(&conn);

  run_with_ids(&conn, vec![500], osm_admin_level::city, 4, NAME_PRIORITY, |_| {});

  assert_eq!(stored_levels(&conn).len(), 1);
}

// 05.06: load_and_send agrupa as coordenadas por relation, ordenando os ways
// por way_order e os nodes pela posicao na lista de refs
#[test]
fn _05_06_load_and_send_groups_coordinates_by_relation_in_order() {
  let conn = pbf_fixtures::memory_db();
  setup_square_relation(&conn);

  let (tx, rx) = std::sync::mpsc::channel();
  let dispatched = load_and_send(&conn, &[500], NAME_PRIORITY, &tx);
  drop(tx);

  assert_eq!(dispatched, 1);
  let work: Vec<_> = rx.into_iter().collect();
  assert_eq!(work.len(), 1);
  assert_eq!(work[0].relation_id, 500);
  assert_eq!(work[0].meta.name, "Lisboa");
  assert_eq!(work[0].ways.len(), 2, "a relation tem 2 ways membros");
  assert_eq!(work[0].ways[0].0.len(), 3, "cada way tem 3 nodes");
  assert!(
    approx_eq(work[0].ways[0].0[0], Coord { x: 0.0, y: 0.0 }),
    "o primeiro way deve comecar no primeiro ref"
  );
}

// 05.07: relation inexistente nao dispara trabalho nenhum
#[test]
fn _05_07_load_and_send_dispatches_nothing_for_unknown_ids() {
  let conn = pbf_fixtures::memory_db();
  let (tx, rx) = std::sync::mpsc::channel();

  assert_eq!(load_and_send(&conn, &[999], NAME_PRIORITY, &tx), 0);
  drop(tx);
  assert!(rx.into_iter().next().is_none());
}
