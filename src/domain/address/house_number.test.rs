use super::enrich_house_numbers;
use crate::domain::address::fixtures::{admin_level_at, match_at};
use crate::domain::address::{admin_level, query_match};
use crate::domain::admin_level::repository::batch_upsert;
use crate::domain::admin_level::{admin_level as admin_levels_row, level};
use crate::domain::house_number::fixtures::link;
use crate::domain::house_number::repository::batch_insert_links;
use geo::{Coord, Geometry, LineString, Point};
use rusqlite::Connection;

fn setup() -> (Connection, i64) {
  let conn = crate::database::open_write(":memory:");
  let ls = LineString(vec![
    Coord { x: 0.0, y: 0.0 },
    Coord { x: 0.0001, y: 0.0001 },
  ]);
  let street = admin_levels_row {
    relation_id: None,
    way_id: Some(1),
    level: level::street,
    wkb: Geometry::LineString(ls).into(),
    name: "rua x".to_string(),
    country_iso_code: None,
    post_code: None,
  };
  batch_upsert(&conn, &[street]);
  // way_id 1 packs to admin id 2; house_numbers.admin_level_id references it.
  (conn, 2)
}

fn insert_hn(conn: &Connection, admin_level_id: i64, node_id: u64, number: &str, lon: f64, lat: f64) {
  batch_insert_links(conn, &[link(node_id, admin_level_id, number, lon, lat)]);
}

fn make_match(admin_level_id: i64, admin_levels: Vec<admin_level>, friendly_name: &str) -> query_match {
  let mut m = match_at(2, 0.0, 0.0, admin_levels);
  m.coordinates_distance_in_meters = Some(0);
  m.friendly_name = friendly_name.to_string();
  m.admin_level_id = Some(admin_level_id);
  m
}

fn level_30_name(m: &query_match) -> Option<&str> {
  m.admin_levels
    .iter()
    .find(|a| a.level == 30)
    .map(|a| a.name.as_str())
}

#[test]
fn _00_no_house_numbers_leaves_matches_unchanged() {
  let (conn, street_id) = setup();
  let mut matches = vec![make_match(street_id, vec![admin_level_at(12, "rua x")], "rua x")];

  enrich_house_numbers(&conn, Point::new(0.0, 0.0), &mut matches, None);

  assert_eq!(matches[0].admin_levels.len(), 1);
  assert_eq!(level_30_name(&matches[0]), None);
  assert_eq!(matches[0].friendly_name, "rua x");
}

#[test]
fn _01_house_number_within_50m_updates_admin_levels_and_friendly_name() {
  let (conn, street_id) = setup();
  insert_hn(&conn, street_id, 1, "123", 0.0, 0.0001); // ~11 m
  let mut matches = vec![make_match(street_id, vec![admin_level_at(12, "rua x")], "rua x")];

  enrich_house_numbers(&conn, Point::new(0.0, 0.0), &mut matches, None);

  assert_eq!(level_30_name(&matches[0]), Some("123"));
  assert_eq!(matches[0].friendly_name, "rua x, 123");
}

#[test]
fn _02_house_number_beyond_50m_is_ignored() {
  let (conn, street_id) = setup();
  insert_hn(&conn, street_id, 1, "123", 0.0, 0.001); // ~111 m
  let mut matches = vec![make_match(street_id, vec![admin_level_at(12, "rua x")], "rua x")];

  enrich_house_numbers(&conn, Point::new(0.0, 0.0), &mut matches, None);

  assert_eq!(matches[0].admin_levels.len(), 1);
  assert_eq!(level_30_name(&matches[0]), None);
  assert_eq!(matches[0].friendly_name, "rua x");
}

#[test]
fn _03_multiple_house_numbers_picks_the_closest() {
  let (conn, street_id) = setup();
  insert_hn(&conn, street_id, 1, "100", 0.0, 0.0003); // ~33 m
  insert_hn(&conn, street_id, 2, "200", 0.0, 0.00005); // ~5.5 m
  let mut matches = vec![make_match(street_id, vec![admin_level_at(12, "rua x")], "rua x")];

  enrich_house_numbers(&conn, Point::new(0.0, 0.0), &mut matches, None);

  assert_eq!(level_30_name(&matches[0]), Some("200"));
}

#[test]
fn _04_template_format_renders_over_enriched_admin_levels() {
  let (conn, street_id) = setup();
  insert_hn(&conn, street_id, 1, "123", 0.0, 0.0001); // ~11 m
  let mut matches = vec![make_match(street_id, vec![admin_level_at(12, "rua x")], "rua x")];

  enrich_house_numbers(
    &conn,
    Point::new(0.0, 0.0),
    &mut matches,
    Some("{admin_level_12_name} {house_number}"),
  );

  // {house_number} resolves against the freshly pushed level-30 entry.
  assert_eq!(matches[0].friendly_name, "rua x 123");
}

#[test]
fn _05_no_format_applies_default_friendly_name_after_level_30_push() {
  let (conn, street_id) = setup();
  insert_hn(&conn, street_id, 1, "123", 0.0, 0.0001); // ~11 m
  let mut matches = vec![make_match(
    street_id,
    vec![admin_level_at(12, "rua x"), admin_level_at(2, "brasil")],
    "rua x",
  )];

  enrich_house_numbers(&conn, Point::new(0.0, 0.0), &mut matches, None);

  // default ordering: street (12), then house number (30), then ancestors.
  assert_eq!(matches[0].friendly_name, "rua x, 123, brasil");
}
