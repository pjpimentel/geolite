use rusqlite::Connection;

use super::super::entity::hierarchy_row;
use super::super::fixtures::{area, relation, street};
use super::{batch_insert, count, destroy, load_by_ids, pending_street_ids, pending_total};
use crate::domain::admin_level::level;
use crate::domain::admin_level::repository::batch_upsert;

const RUA_A: i64 = 2;
const RUA_B: i64 = 4;
const CIDADE: i64 = 15;

fn row(id: i64, chain: &str, label: &str) -> hierarchy_row {
  hierarchy_row {
    admin_level_id: id,
    ancestor_ids: chain.to_string(),
    user_friendly_name: label.to_string(),
  }
}

// two streets and a city, so that the pending queries have both kinds to tell apart
fn seeded() -> Connection {
  let conn = crate::database::open_write(":memory:");
  batch_upsert(
    &conn,
    &[
      street(1, "Rua A", 0.0),
      street(2, "Rua B", 1.0),
      area(7, level::city, "Cidade", 0.0, 10.0),
    ],
  );
  assert_eq!(relation(7), CIDADE);
  conn
}

#[test]
fn _00_a_fresh_database_has_an_empty_hierarchy() {
  let conn = crate::database::open_write(":memory:");
  assert_eq!(count(&conn), 0);
  assert!(load_by_ids(&conn, &[RUA_A]).is_empty());
  assert!(load_by_ids(&conn, &[]).is_empty());
}

#[test]
fn _01_batch_insert_counts_rows_and_ignores_a_repeated_id() {
  let conn = seeded();
  batch_insert(&conn, &[row(RUA_A, "[15]", "Rua A, Cidade"), row(CIDADE, "[]", "Cidade")]);
  batch_insert(&conn, &[row(RUA_A, "[]", "again")]);
  batch_insert(&conn, &[]);

  assert_eq!(count(&conn), 2);
  let rows = load_by_ids(&conn, &[RUA_A]);
  assert_eq!(rows[&RUA_A].user_friendly_name, "Rua A, Cidade", "the first row wins");
}

#[test]
fn _02_load_by_ids_decodes_the_chain_and_omits_missing_ids() {
  let conn = seeded();
  batch_insert(&conn, &[row(RUA_A, "[15]", "Rua A, Cidade"), row(CIDADE, "[]", "Cidade")]);

  let rows = load_by_ids(&conn, &[RUA_A, RUA_B, CIDADE]);
  assert_eq!(rows.len(), 2, "Rua B has no hierarchy row yet");
  assert_eq!(rows[&RUA_A].ancestor_ids, vec![CIDADE]);
  assert!(rows[&CIDADE].ancestor_ids.is_empty());
}

#[test]
fn _03_pending_total_counts_every_area_without_a_hierarchy_row() {
  let conn = seeded();
  assert_eq!(pending_total(&conn), 3);
  batch_insert(&conn, &[row(RUA_A, "[15]", "Rua A, Cidade")]);
  assert_eq!(pending_total(&conn), 2);
}

#[test]
fn _04_pending_street_ids_lists_only_the_streets_left() {
  let conn = seeded();
  assert_eq!(pending_street_ids(&conn), vec![RUA_A, RUA_B], "the city is not a street");
  batch_insert(&conn, &[row(RUA_A, "[15]", "Rua A, Cidade")]);
  assert_eq!(pending_street_ids(&conn), vec![RUA_B]);
}

#[test]
fn _05_destroy_empties_the_table_and_keeps_it_usable() {
  let conn = seeded();
  batch_insert(&conn, &[row(RUA_A, "[]", "Rua A")]);
  destroy(&conn);
  assert_eq!(count(&conn), 0);
  batch_insert(&conn, &[row(RUA_B, "[]", "Rua B")]);
  assert_eq!(count(&conn), 1);
}
