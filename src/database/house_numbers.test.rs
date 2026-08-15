use super::*;
use rusqlite::Connection;

// what the number itself means — trimming, canonical suffix, drop values — is covered by pure
// tests in src/domain/house_number/house_number.test.rs. what belongs here is the adapter's own
// job: selecting the right tags out of the payload and handing every raw value to the policy.

fn setup_db() -> Connection {
  let conn = crate::database::open_write(":memory:");
  conn
    .execute_batch("PRAGMA foreign_keys = OFF;")
    .expect("failed to disable fk");
  conn
}

fn make_node(id: u64, tags: Vec<(&str, &str)>) -> crate::database::osm_nodes::osm_node_row {
  let node = crate::extract::osm_data::osm_nodes::osm_node {
    id: id as i64,
    lat: -23.5505,
    lon: -46.6333,
    tags: tags
      .into_iter()
      .map(|(k, v)| (k.to_string(), v.to_string()))
      .collect(),
  };
  let mut payload = Vec::new();
  crate::extract::osm_data::jsonb_encode::encoder::new().encode_osm_node(&mut payload, &node);
  crate::database::osm_nodes::osm_node_row {
    id,
    osm_pbf_chunk_id: 0,
    payload,
  }
}

fn policy(
  number_tags: &'static [&'static str],
  street_tags: &'static [&'static str],
  drop_values: &'static [&'static str],
) -> house_number_policy {
  house_number_policy {
    number_tags,
    street_tags,
    drop_values,
    ..crate::presets::resolve(None).house_numbers
  }
}

// inserts the given nodes and returns the candidates produced by the extraction query.
fn load(
  nodes: &[(u64, &[(&str, &str)])],
  policy: &house_number_policy,
) -> Vec<candidate_row> {
  let conn = setup_db();
  let rows: Vec<_> = nodes
    .iter()
    .map(|(id, tags)| make_node(*id, tags.to_vec()))
    .collect();
  crate::database::osm_nodes::insert_rows(&conn, &rows);
  load_all_candidates(&conn, policy)
}

const HN: &[&str] = &["addr:housenumber"];
const ST: &[&str] = &["addr:street"];
const NO_DROPS: &[&str] = &[];
const BR_DROPS: &[&str] = &["s/n", "sn", "s/nº", "s/no"];

#[test]
fn _00_the_tag_value_reaches_the_candidate_in_its_stored_form() {
  let c = load(&[(1, &[("addr:housenumber", "  12 a  ")])], &policy(HN, ST, NO_DROPS));
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
  // the drop list and the blank check now run in rust, over every row the query returns.
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
  // without the fallback tag the node is not a candidate
  let c = load(nodes, &policy(HN, ST, NO_DROPS));
  assert_eq!(c.len(), 0);
  // with the fallback tag the value is picked up
  const WITH_FALLBACK: &[&str] = &["addr:housenumber", "addr:conscriptionnumber"];
  let c = load(nodes, &policy(WITH_FALLBACK, ST, NO_DROPS));
  assert_eq!(c.len(), 1);
  assert_eq!(c[0].number.stored_form(), "42");
}

#[test]
fn _05_street_tag_fallback_is_used() {
  let nodes: &[(u64, &[(&str, &str)])] =
    &[(1, &[("addr:housenumber", "10"), ("addr:place", "Plaza")])];
  // without the fallback tag there is no street
  let c = load(nodes, &policy(HN, ST, NO_DROPS));
  assert_eq!(c[0].addr_street, None);
  // with the fallback tag the place is used as street
  const WITH_FALLBACK: &[&str] = &["addr:street", "addr:place"];
  let c = load(nodes, &policy(HN, WITH_FALLBACK, NO_DROPS));
  assert_eq!(c[0].addr_street.as_deref(), Some("Plaza"));
}

#[test]
fn _06_streets_with_centroid_reads_mbr_center_of_each_street() {
  use crate::database::admin_levels::{admin_levels as admin_levels_row, batch_upsert};
  use geo::{Coord, Geometry, LineString};

  let conn = setup_db();
  let street = |way_id: u64, name: &str, a: (f64, f64), b: (f64, f64)| admin_levels_row {
    relation_id: None,
    way_id: Some(way_id),
    admin_level: 12,
    wkb: Geometry::LineString(LineString(vec![
      Coord { x: a.0, y: a.1 },
      Coord { x: b.0, y: b.1 },
    ]))
    .into(),
    name: name.to_string(),
    country_iso_code: None,
    post_code: None,
  };
  // bbox centers: street_a -> (1, 2); street_b -> (15, 25).
  batch_upsert(
    &conn,
    &[
      street(1, "street_a", (0.0, 0.0), (2.0, 4.0)),
      street(2, "street_b", (10.0, 20.0), (20.0, 30.0)),
    ],
  );

  let mut rows = streets_with_centroid(&conn);
  rows.sort_by(|a, b| a.name.cmp(&b.name));
  assert_eq!(rows.len(), 2);
  assert_eq!(rows[0].name, "street_a");
  assert!((rows[0].cx - 1.0).abs() < 1e-9 && (rows[0].cy - 2.0).abs() < 1e-9);
  assert_eq!(rows[1].name, "street_b");
  assert!((rows[1].cx - 15.0).abs() < 1e-9 && (rows[1].cy - 25.0).abs() < 1e-9);
}
