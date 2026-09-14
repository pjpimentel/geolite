use super::{batch_insert_links, by_admin_level_ids, candidate_row, load_all_candidates};
use crate::domain::house_number::{house_number, house_number_policy};

use crate::domain::admin_level::repository::batch_upsert;
use crate::domain::house_number::fixtures::{BR_DROPS, link, simple_policy, street_along};
use crate::domain::pbf_fixtures::{insert_node, memory_db};

const HN: &[&str] = &["addr:housenumber"];
const ST: &[&str] = &["addr:street"];
const NO_DROPS: &[&str] = &[];

fn policy(
  number_tags: &'static [&'static str],
  street_tags: &'static [&'static str],
  drop_values: &'static [&'static str],
) -> house_number_policy {
  house_number_policy {
    number_tags,
    street_tags,
    drop_values,
    ..simple_policy()
  }
}

fn load(nodes: &[(u64, &[(&str, &str)])], policy: &house_number_policy) -> Vec<candidate_row> {
  let conn = memory_db();
  for (id, tags) in nodes {
    insert_node(&conn, *id, -46.6333, -23.5505, tags);
  }
  load_all_candidates(&conn, policy)
}

#[test]
fn _00_the_tag_value_reaches_the_candidate_in_its_stored_form() {
  let c = load(
    &[(1, &[("addr:housenumber", "  12 a  ")])],
    &policy(HN, ST, NO_DROPS),
  );
  assert_eq!(c.len(), 1);
  assert_eq!(c[0].number.stored_form(), "12A");
}

#[test]
fn _01_nodes_without_the_number_tag_are_not_candidates() {
  let c = load(&[(1, &[("addr:street", "Rua X")])], &policy(HN, ST, NO_DROPS));
  assert!(c.is_empty());
}

#[test]
fn _02_values_rejected_by_the_policy_are_dropped() {
  let nodes: &[(u64, &[(&str, &str)])] = &[
    (1, &[("addr:housenumber", "s/n")]),
    (2, &[("addr:housenumber", "S/N")]),
    (3, &[("addr:housenumber", "  s/n  ")]),
    (4, &[("addr:housenumber", "   ")]),
    (5, &[("addr:housenumber", "100")]),
  ];
  let c = load(nodes, &policy(HN, ST, BR_DROPS));
  assert_eq!(c.len(), 1);
  assert_eq!(c[0].number.stored_form(), "100");
}

#[test]
fn _03_drop_values_are_kept_when_the_drop_list_is_empty() {
  let c = load(&[(1, &[("addr:housenumber", "s/n")])], &policy(HN, ST, NO_DROPS));
  assert_eq!(c.len(), 1);
  assert_eq!(c[0].number.stored_form(), "s/n");
}

#[test]
fn _04_housenumber_tag_fallback_is_used() {
  let nodes: &[(u64, &[(&str, &str)])] = &[(1, &[("addr:conscriptionnumber", "42")])];
  let c = load(nodes, &policy(HN, ST, NO_DROPS));
  assert_eq!(c.len(), 0);
  const WITH_FALLBACK: &[&str] = &["addr:housenumber", "addr:conscriptionnumber"];
  let c = load(nodes, &policy(WITH_FALLBACK, ST, NO_DROPS));
  assert_eq!(c.len(), 1);
  assert_eq!(c[0].number.stored_form(), "42");
}

#[test]
fn _05_street_tag_fallback_is_used() {
  let nodes: &[(u64, &[(&str, &str)])] =
    &[(1, &[("addr:housenumber", "10"), ("addr:place", "Plaza")])];
  let c = load(nodes, &policy(HN, ST, NO_DROPS));
  assert_eq!(c[0].addr_street, None);
  const WITH_FALLBACK: &[&str] = &["addr:street", "addr:place"];
  let c = load(nodes, &policy(HN, WITH_FALLBACK, NO_DROPS));
  assert_eq!(c[0].addr_street.as_deref(), Some("Plaza"));
}

#[test]
fn _06_batch_insert_links_ignores_a_node_already_linked() {
  let conn = memory_db();
  batch_upsert(&conn, &[street_along(1, "rua x", &[(0.0, 0.0), (1.0, 0.0)])]);
  let first = batch_insert_links(&conn, &[link(7, 2, "12A", 0.0, 0.0)]);
  let again = batch_insert_links(&conn, &[link(7, 2, "12a", 1.0, 1.0)]);
  assert_eq!((first, again), (1, 0));

  let rows = by_admin_level_ids(&conn, &[2]);
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].number, house_number::from_stored("12A"));
  assert_eq!(rows[0].number.stored_form(), "12A");
}
