use geo::{Coord, Geometry, LineString, Winding};
use rusqlite::Connection;

use super::super::geometry::{admin_geometry, approx_eq};
use super::super::id::admin_level_id;
use super::super::repository::load_all_below_street;
use super::super::scale::level;
use super::{load_and_send, process_one_relation, rel_meta, run_with_ids};
use crate::domain::pbf_fixtures::{self, NAME_PRIORITY};

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

// 00.00: relation cujas ways estao todas vazias e descartada
#[test]
fn _00_00_relation_with_only_empty_ways_is_skipped() {
  let result = process_one_relation(1, &meta(), &[ls(&[]), ls(&[])], level::city);
  assert!(result.is_none());
}

// 00.01: anel fechado com >=4 pontos vira MultiPolygon com winding CW
// (replica o comportamento do st_buildarea do spatialite)
#[test]
fn _00_01_closed_ring_becomes_clockwise_multipolygon() {
  // quadrado em ordem CCW — process_one_relation deve inverter para CW
  let ways = [ls(&[
    (0.0, 0.0),
    (1.0, 0.0),
    (1.0, 1.0),
    (0.0, 1.0),
    (0.0, 0.0),
  ])];
  let row = process_one_relation(7, &meta(), &ways, level::city)
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

// 00.02: anel aberto nao vira poligono — cai no MultiLineString
#[test]
fn _00_02_open_ring_becomes_multilinestring() {
  let ways = [ls(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)])];
  let row = process_one_relation(8, &meta(), &ways, level::city)
    .expect("anel aberto ainda produz uma linha");

  assert!(
    matches!(row.wkb.geometry(), Geometry::MultiLineString(_)),
    "anel nao fechado deve virar MultiLineString"
  );
}

// 00.03: anel fechado com menos de 4 pontos nao satisfaz o minimo de poligono
#[test]
fn _00_03_closed_ring_with_too_few_points_is_not_a_polygon() {
  let ways = [ls(&[(0.0, 0.0), (1.0, 0.0), (0.0, 0.0)])];
  let row = process_one_relation(9, &meta(), &ways, level::city)
    .expect("deve produzir linha mesmo sem virar poligono");

  assert!(
    matches!(row.wkb.geometry(), Geometry::MultiLineString(_)),
    "3 pontos nao bastam para um poligono"
  );
}

// 00.04: metadados e admin_level sao propagados para a linha resultante
#[test]
fn _00_04_propagates_metadata_and_admin_level() {
  let ways = [ls(&[(0.0, 0.0), (1.0, 0.0)])];
  let row = process_one_relation(42, &meta(), &ways, level::municipality)
    .expect("deve produzir uma linha");

  assert_eq!(row.relation_id, Some(42));
  assert_eq!(row.way_id, None);
  assert_eq!(row.level, level::municipality);
  assert_eq!(row.name, "Lisboa");
  assert_eq!(row.country_iso_code.as_deref(), Some("PT"));
  assert_eq!(row.post_code.as_deref(), Some("1000-001"));
}

/////////////////////////////////////////////////////////////////////////////////
// 01 — run_with_ids e load_and_send ponta a ponta
/////////////////////////////////////////////////////////////////////////////////

// relation 500 formada por 2 ways que juntos fecham um quadrado
fn setup_square_relation(conn: &Connection) {
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

fn stored_levels(conn: &Connection) -> Vec<(Option<u64>, u8, String)> {
  load_all_below_street(conn)
    .into_iter()
    .map(|row| {
      (
        Some(admin_level_id::from_raw(row.id as u64).osm_id()),
        row.admin_level.value(),
        row.name,
      )
    })
    .collect()
}

// 01.00: sem ids a processar o estagio encerra cedo, reportando total zero
#[test]
fn _01_00_returns_early_when_the_id_list_is_empty() {
  let conn = pbf_fixtures::memory_db();

  let seen = std::cell::RefCell::new(Vec::new());
  run_with_ids(
    &conn,
    Vec::new(),
    level::city,
    1,
    NAME_PRIORITY,
    |p| seen.borrow_mut().push((p.total, p.processed)),
  );

  assert_eq!(seen.into_inner(), vec![(Some(0), 0)]);
  assert!(stored_levels(&conn).is_empty());
}

// 01.01: relation com ways que fecham um anel vira MultiPolygon persistido,
// carregando nome, codigo de pais e codigo postal vindos das tags
#[test]
fn _01_01_upserts_a_multipolygon_for_a_closed_relation() {
  let conn = pbf_fixtures::memory_db();
  setup_square_relation(&conn);

  let seen = std::cell::RefCell::new(Vec::new());
  run_with_ids(
    &conn,
    vec![500],
    level::city,
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

// 01.02: relation cujos ways nao fecham anel vira MultiLineString
#[test]
fn _01_02_falls_back_to_multilinestring_for_open_relations() {
  let conn = pbf_fixtures::memory_db();
  pbf_fixtures::insert_node(&conn, 1, 0.0, 0.0, &[]);
  pbf_fixtures::insert_node(&conn, 2, 1.0, 0.0, &[]);
  pbf_fixtures::insert_node(&conn, 3, 2.0, 1.0, &[]);
  pbf_fixtures::insert_way(&conn, 10, &[1, 2, 3], &[("name", "Trecho aberto")]);
  pbf_fixtures::insert_relation(&conn, 500, &[(1, 10, "outer")], &[("name", "Aberta")]);

  run_with_ids(&conn, vec![500], level::city, 1, NAME_PRIORITY, |_| {});

  let wkb: admin_geometry = conn
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

// 01.03: membros que nao sao way sao ignorados pela query de coordenadas
#[test]
fn _01_03_ignores_relation_members_that_are_not_ways() {
  let conn = pbf_fixtures::memory_db();
  setup_square_relation(&conn);
  pbf_fixtures::insert_relation(
    &conn,
    501,
    &[(0, 1, "admin_centre"), (2, 500, "subarea")],
    &[("name", "So membros nao-way")],
  );

  run_with_ids(&conn, vec![501], level::city, 1, NAME_PRIORITY, |_| {});

  assert!(
    stored_levels(&conn).is_empty(),
    "sem way nenhum nao ha geometria a montar"
  );
}

// 01.04: several relations are processed and progress accumulates
#[test]
fn _01_04_processes_several_relations_and_accumulates_progress() {
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
    level::city,
    1,
    NAME_PRIORITY,
    |p| seen.borrow_mut().push(p.processed),
  );

  assert_eq!(stored_levels(&conn).len(), 2);
  assert_eq!(seen.into_inner().last().copied(), Some(2));
}

// 01.05: com mais de uma thread o resultado continua o mesmo
#[test]
fn _01_05_produces_the_same_rows_with_multiple_threads() {
  let conn = pbf_fixtures::memory_db();
  setup_square_relation(&conn);

  run_with_ids(&conn, vec![500], level::city, 4, NAME_PRIORITY, |_| {});

  assert_eq!(stored_levels(&conn).len(), 1);
}

// 01.06: load_and_send agrupa as coordenadas por relation, ordenando os ways
// por way_order e os nodes pela posicao na lista de refs
#[test]
fn _01_06_load_and_send_groups_coordinates_by_relation_in_order() {
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

// 01.07: relation inexistente nao dispara trabalho nenhum
#[test]
fn _01_07_load_and_send_dispatches_nothing_for_unknown_ids() {
  let conn = pbf_fixtures::memory_db();
  let (tx, rx) = std::sync::mpsc::channel();

  assert_eq!(load_and_send(&conn, &[999], NAME_PRIORITY, &tx), 0);
  drop(tx);
  assert!(rx.into_iter().next().is_none());
}
