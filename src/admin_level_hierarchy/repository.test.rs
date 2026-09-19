use std::collections::HashMap;

use rusqlite::Connection;

use super::super::entity::hierarchy_edges;
use super::super::fixtures::{area, relation, street};
use super::{
  SQL_NODES_UNDER, admin_levels_hierarchy, ancestry_of, batch_insert, children_of, count, destroy,
  load_all_edges, parents_of, pending_street_ids, pending_total, roots,
};
use crate::admin_level::level;
use crate::admin_level::repository::batch_upsert;
use crate::database::table;

const RUA_A: i64 = 2;
const RUA_B: i64 = 4;
const CIDADE: i64 = 15;
const PAIS: i64 = 19;

fn edges(id: i64, parents: &[i64]) -> hierarchy_edges {
  hierarchy_edges {
    admin_level_id: id,
    parents: parents.to_vec(),
  }
}

// two streets, a city and a country, so that the pending queries have both kinds to tell apart
fn seeded() -> Connection {
  let conn = crate::database::open_write(":memory:");
  batch_upsert(
    &conn,
    &[
      street(1, "Rua A", 0.0),
      street(2, "Rua B", 1.0),
      area(7, level::city, "Cidade", 0.0, 10.0),
      area(9, level::country, "Pais", 0.0, 100.0),
    ],
  );
  assert_eq!((relation(7), relation(9)), (CIDADE, PAIS));
  conn
}

fn chained() -> Connection {
  let conn = seeded();
  batch_insert(
    &conn,
    &[edges(RUA_A, &[CIDADE]), edges(CIDADE, &[PAIS]), edges(PAIS, &[])],
  );
  conn
}

fn ids(nodes: &[super::super::entity::node]) -> Vec<(i64, level, String)> {
  nodes
    .iter()
    .map(|n| (n.id, n.level, n.name.clone()))
    .collect()
}

fn plan(conn: &Connection, parent_id: Option<i64>) -> Vec<String> {
  conn
    .prepare(&format!("EXPLAIN QUERY PLAN {SQL_NODES_UNDER}"))
    .expect("failed to explain the nodes query")
    .query_map([parent_id], |row| row.get::<_, String>(3))
    .expect("failed to read the plan")
    .collect::<Result<_, _>>()
    .expect("failed to collect the plan")
}

#[test]
fn _00_a_fresh_database_has_an_empty_hierarchy() {
  let conn = crate::database::open_write(":memory:");
  assert_eq!(count(&conn), 0);
  assert!(ancestry_of(&conn, &[RUA_A]).is_empty());
  assert!(ancestry_of(&conn, &[]).is_empty());
  assert!(roots(&conn).is_empty());
  assert!(load_all_edges(&conn).is_empty());
}

#[test]
fn _01_batch_insert_writes_one_row_per_parent_and_one_row_for_a_root() {
  let conn = chained();
  assert_eq!(count(&conn), 3);

  batch_insert(
    &conn,
    &[edges(RUA_A, &[CIDADE]), edges(CIDADE, &[PAIS]), edges(PAIS, &[])],
  );
  batch_insert(&conn, &[]);

  assert_eq!(count(&conn), 3, "a repeated edge and a repeated root are ignored");
}

#[test]
fn _02_ancestry_of_climbs_to_the_roots_in_layers() {
  let conn = chained();

  let edges = ancestry_of(&conn, &[RUA_A]);

  let expected: HashMap<i64, Vec<i64>> =
    HashMap::from([(RUA_A, vec![CIDADE]), (CIDADE, vec![PAIS]), (PAIS, vec![])]);
  assert_eq!(edges, expected);
  assert!(ancestry_of(&conn, &[RUA_B]).is_empty(), "Rua B has no row yet");
  assert_eq!(ancestry_of(&conn, &[CIDADE]).len(), 2);
}

#[test]
fn _03_pending_total_counts_every_area_without_a_row() {
  let conn = seeded();
  assert_eq!(pending_total(&conn), 4);
  batch_insert(&conn, &[edges(RUA_A, &[CIDADE])]);
  assert_eq!(pending_total(&conn), 3);
  batch_insert(&conn, &[edges(PAIS, &[])]);
  assert_eq!(pending_total(&conn), 2, "a root row resolves the area too");
}

#[test]
fn _04_pending_street_ids_lists_only_the_streets_left() {
  let conn = seeded();
  assert_eq!(pending_street_ids(&conn), vec![RUA_A, RUA_B], "the areas are not streets");
  batch_insert(&conn, &[edges(RUA_A, &[CIDADE])]);
  assert_eq!(pending_street_ids(&conn), vec![RUA_B]);
}

#[test]
fn _05_destroy_empties_the_table_and_keeps_it_usable() {
  let conn = chained();
  destroy(&conn);
  assert_eq!(count(&conn), 0);
  batch_insert(&conn, &[edges(RUA_B, &[])]);
  assert_eq!(count(&conn), 1);
}

#[test]
fn _06_children_of_lists_a_child_once_per_parent_by_level_and_name() {
  let conn = seeded();
  batch_insert(
    &conn,
    &[edges(RUA_A, &[CIDADE, PAIS]), edges(CIDADE, &[PAIS]), edges(PAIS, &[])],
  );

  assert_eq!(
    ids(&children_of(&conn, PAIS)),
    vec![
      (CIDADE, level::city, "Cidade".to_string()),
      (RUA_A, level::street, "Rua A".to_string())
    ]
  );
  assert_eq!(ids(&children_of(&conn, CIDADE)), vec![(RUA_A, level::street, "Rua A".to_string())]);
  assert!(children_of(&conn, RUA_B).is_empty());
}

#[test]
fn _07_roots_are_the_rows_without_a_parent() {
  let conn = chained();
  assert_eq!(ids(&roots(&conn)), vec![(PAIS, level::country, "Pais".to_string())]);

  batch_insert(&conn, &[edges(RUA_B, &[])]);

  assert_eq!(
    ids(&roots(&conn)),
    vec![
      (PAIS, level::country, "Pais".to_string()),
      (RUA_B, level::street, "Rua B".to_string())
    ],
    "a street nobody contains is a root too"
  );
}

#[test]
fn _08_parents_of_lists_the_direct_parents_in_id_order() {
  let conn = seeded();
  batch_insert(&conn, &[edges(RUA_A, &[PAIS, CIDADE]), edges(PAIS, &[])]);

  assert_eq!(parents_of(&conn, RUA_A), vec![CIDADE, PAIS]);
  assert!(parents_of(&conn, PAIS).is_empty());
  assert!(parents_of(&conn, RUA_B).is_empty());
}

#[test]
fn _09_children_and_roots_read_the_search_by_parent_index() {
  let conn = chained();
  admin_levels_hierarchy::create_indexes(&conn);

  for parent_id in [Some(CIDADE), None] {
    let steps = plan(&conn, parent_id);
    assert!(
      steps
        .iter()
        .any(|step| step.contains("INDEX admin_levels_hierarchy_search_by_parent")),
      "parent {parent_id:?}: {steps:?}"
    );
  }
}

#[test]
fn _10_load_all_edges_reads_the_whole_table_with_the_roots() {
  let conn = chained();
  batch_insert(&conn, &[edges(RUA_B, &[CIDADE, PAIS])]);

  let expected: HashMap<i64, Vec<i64>> = HashMap::from([
    (RUA_A, vec![CIDADE]),
    (RUA_B, vec![CIDADE, PAIS]),
    (CIDADE, vec![PAIS]),
    (PAIS, vec![]),
  ]);
  assert_eq!(load_all_edges(&conn), expected);
}
