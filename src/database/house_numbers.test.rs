use super::*;
use rusqlite::Connection;

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

// inserts the given nodes and returns the candidates produced by the extraction query.
fn load(
  nodes: &[(u64, &[(&str, &str)])],
  housenumber_tags: &[&str],
  street_tags: &[&str],
  drop_values: &[&str],
) -> Vec<candidate_row> {
  let conn = setup_db();
  let rows: Vec<_> = nodes
    .iter()
    .map(|(id, tags)| make_node(*id, tags.to_vec()))
    .collect();
  crate::database::osm_nodes::insert_rows(&conn, &rows);
  load_all_candidates(&conn, housenumber_tags, street_tags, drop_values)
}

const HN: &[&str] = &["addr:housenumber"];
const ST: &[&str] = &["addr:street"];
const NO_DROPS: &[&str] = &[];
const BR_DROPS: &[&str] = &["s/n", "sn", "s/nº", "s/no"];

#[test]
fn _00_pure_number_is_unchanged() {
  let c = load(&[(1, &[("addr:housenumber", "100")])], HN, ST, NO_DROPS);
  assert_eq!(c.len(), 1);
  assert_eq!(c[0].number, "100");
}

#[test]
fn _01_leading_and_trailing_whitespace_is_trimmed() {
  let c = load(&[(1, &[("addr:housenumber", "  100  ")])], HN, ST, NO_DROPS);
  assert_eq!(c.len(), 1);
  assert_eq!(c[0].number, "100");
}

#[test]
fn _02_space_separated_suffix_is_canonicalized() {
  let c = load(&[(1, &[("addr:housenumber", "12 a")])], HN, ST, NO_DROPS);
  assert_eq!(c[0].number, "12A");
}

#[test]
fn _03_hyphen_separated_suffix_is_canonicalized() {
  let c = load(&[(1, &[("addr:housenumber", "12-a")])], HN, ST, NO_DROPS);
  assert_eq!(c[0].number, "12A");
}

#[test]
fn _04_attached_suffix_is_canonicalized() {
  let c = load(&[(1, &[("addr:housenumber", "12a")])], HN, ST, NO_DROPS);
  assert_eq!(c[0].number, "12A");
}

#[test]
fn _05_already_uppercase_suffix_is_preserved() {
  let c = load(&[(1, &[("addr:housenumber", "12A")])], HN, ST, NO_DROPS);
  assert_eq!(c[0].number, "12A");
}

#[test]
fn _06_numeric_range_is_unchanged() {
  let c = load(&[(1, &[("addr:housenumber", "12-14")])], HN, ST, NO_DROPS);
  assert_eq!(c[0].number, "12-14");
}

#[test]
fn _07_non_house_number_strings_are_unchanged() {
  let c = load(
    &[
      (1, &[("addr:housenumber", "Lote 5")]),
      (2, &[("addr:housenumber", "Fundos")]),
    ],
    HN,
    ST,
    NO_DROPS,
  );
  let mut numbers: Vec<&str> = c.iter().map(|r| r.number.as_str()).collect();
  numbers.sort();
  assert_eq!(numbers, vec!["Fundos", "Lote 5"]);
}

#[test]
fn _08_drop_values_are_discarded_case_insensitively_after_trim() {
  let c = load(
    &[
      (1, &[("addr:housenumber", "s/n")]),
      (2, &[("addr:housenumber", "S/N")]),
      (3, &[("addr:housenumber", "  s/n  ")]),
    ],
    HN,
    ST,
    BR_DROPS,
  );
  assert_eq!(c.len(), 0);
}

#[test]
fn _09_drop_values_are_kept_when_drop_list_is_empty() {
  let c = load(&[(1, &[("addr:housenumber", "s/n")])], HN, ST, NO_DROPS);
  assert_eq!(c.len(), 1);
  assert_eq!(c[0].number, "s/n");
}

#[test]
fn _10_housenumber_tag_fallback_is_used() {
  let nodes: &[(u64, &[(&str, &str)])] = &[(1, &[("addr:conscriptionnumber", "42")])];
  // without the fallback tag the node is not a candidate
  let c = load(nodes, &["addr:housenumber"], ST, NO_DROPS);
  assert_eq!(c.len(), 0);
  // with the fallback tag the value is picked up
  let c = load(nodes, &["addr:housenumber", "addr:conscriptionnumber"], ST, NO_DROPS);
  assert_eq!(c.len(), 1);
  assert_eq!(c[0].number, "42");
}

#[test]
fn _12_streets_with_centroid_reads_mbr_center_of_each_street() {
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

#[test]
fn _11_street_tag_fallback_is_used() {
  let nodes: &[(u64, &[(&str, &str)])] =
    &[(1, &[("addr:housenumber", "10"), ("addr:place", "Plaza")])];
  // without the fallback tag there is no street
  let c = load(nodes, HN, &["addr:street"], NO_DROPS);
  assert_eq!(c[0].addr_street, None);
  // with the fallback tag the place is used as street
  let c = load(nodes, HN, &["addr:street", "addr:place"], NO_DROPS);
  assert_eq!(c[0].addr_street.as_deref(), Some("Plaza"));
}
