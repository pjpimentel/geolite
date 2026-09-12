use crate::domain::house_number::house_number_link;

use crate::domain::admin_level::repository::batch_upsert;
use crate::domain::house_number::fixtures::{simple_policy, stored_links, street_along};
use crate::domain::house_number::strategy::link_strategy;
use crate::domain::pbf_fixtures::{insert_node, memory_db};
use geo::{Geometry, Point};

fn insert_street(conn: &rusqlite::Connection, id: u64, name: &str, points: &[(f64, f64)]) {
  batch_upsert(conn, &[street_along(id, name, points)]);
}

fn extracted(conn: &rusqlite::Connection) -> Vec<(u64, u64)> {
  let mut seen = Vec::new();
  house_number_link::extract(conn, &simple_policy(), |p| {
    seen.push((p.total, p.processed))
  });
  seen
}

// 00: sem candidato nenhum o run encerra cedo reportando total zero
#[test]
fn _00_returns_early_when_there_are_no_candidates() {
  let conn = memory_db();
  insert_street(&conn, 1, "Rua Vazia", &[(0.0, 0.0), (1.0, 0.0)]);

  assert_eq!(extracted(&conn), vec![(0, 0)]);
  assert!(stored_links(&conn).is_empty());
}

// 01: fluxo completo — casa por proximidade e grava a linha
#[test]
fn _01_matches_and_persists_house_numbers_by_proximity() {
  let conn = memory_db();
  insert_street(&conn, 1, "Rua Augusta", &[(0.0, 0.0), (1.0, 0.0)]);
  insert_node(&conn, 10, 0.5, 0.001, &[("addr:housenumber", "100")]);

  let seen = extracted(&conn);

  let rows = stored_links(&conn);
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].0, 10, "node_id");
  assert_eq!(rows[0].2, "100", "number");
  assert_eq!(rows[0].3, link_strategy::by_proximity.code() as i64);

  assert_eq!(seen.first().expect("evento inicial"), &(1, 0));
  assert_eq!(seen.last().expect("evento final"), &(1, 1));
}

// 02: addr:street presente muda a estrategia para casamento por nome
#[test]
fn _02_matches_by_name_when_addr_street_is_present() {
  let conn = memory_db();
  insert_street(&conn, 1, "Rua Perto", &[(0.0, 0.0), (1.0, 0.0)]);
  insert_street(&conn, 2, "Rua Nomeada", &[(0.0, 0.1), (1.0, 0.1)]);
  insert_node(
    &conn,
    10,
    0.5,
    0.001,
    &[("addr:housenumber", "100"), ("addr:street", "Rua Nomeada")],
  );

  extracted(&conn);

  let rows = stored_links(&conn);
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].3, link_strategy::by_name.code() as i64);
}

// 03: rua no tile vizinho ainda e considerada, gracas a expansao de +-1 tile
#[test]
fn _03_considers_streets_from_neighbouring_tiles() {
  let conn = memory_db();
  // TILE_SIZE e 2.0: a rua fica no tile x=0 e o endereco no tile x=1
  insert_street(&conn, 1, "Rua da Fronteira", &[(1.9, 0.0), (2.1, 0.0)]);
  insert_node(&conn, 10, 2.05, 0.001, &[("addr:housenumber", "100")]);

  extracted(&conn);

  let rows = stored_links(&conn);
  assert_eq!(
    rows.len(),
    1,
    "a rua do tile vizinho deveria entrar na busca"
  );
  assert_eq!(rows[0].1, stored_links(&conn)[0].1);
}

// 04: candidates spread across several tiles are all processed
#[test]
fn _04_processes_candidates_spread_across_tiles() {
  let conn = memory_db();
  insert_street(&conn, 1, "Rua Um", &[(0.0, 0.0), (1.0, 0.0)]);
  insert_street(&conn, 2, "Rua Dois", &[(10.0, 10.0), (11.0, 10.0)]);
  insert_node(&conn, 10, 0.5, 0.001, &[("addr:housenumber", "100")]);
  insert_node(&conn, 20, 10.5, 10.001, &[("addr:housenumber", "200")]);

  extracted(&conn);

  let rows = stored_links(&conn);
  assert_eq!(rows.len(), 2);
  assert_eq!(rows[0].2, "100");
  assert_eq!(rows[1].2, "200");
}

// 05: endereco distante de qualquer rua nao vira linha
#[test]
fn _05_skips_candidates_with_no_street_within_range() {
  let conn = memory_db();
  insert_street(&conn, 1, "Rua Distante", &[(0.0, 0.0), (1.0, 0.0)]);
  insert_node(&conn, 10, 50.0, 50.0, &[("addr:housenumber", "100")]);

  extracted(&conn);

  assert!(stored_links(&conn).is_empty());
}

// 06: rua cuja geometria nao e linha nem area e ignorada no carregamento
#[test]
fn _06_skips_streets_whose_geometry_is_not_linear() {
  let conn = memory_db();
  // rua gravada como ponto: geometry_to_multilinestring devolve None
  batch_upsert(
    &conn,
    &[crate::domain::admin_level::admin_level {
      relation_id: None,
      way_id: Some(1),
      level: crate::domain::admin_level::level::street,
      name: "Rua Pontual".to_string(),
      country_iso_code: None,
      post_code: None,
      wkb: Geometry::Point(Point::new(0.0, 0.0)).into(),
    }],
  );
  insert_node(&conn, 10, 0.001, 0.001, &[("addr:housenumber", "100")]);

  extracted(&conn);

  assert!(
    stored_links(&conn).is_empty(),
    "rua sem geometria linear nao pode casar endereco"
  );
}
