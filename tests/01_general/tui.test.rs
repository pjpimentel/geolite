use crate::admin_level_hierarchy::{area, resolved_at, street};
use crate::common::leaf::{MAP_COLUMNS, MAP_ROWS, assert_the_map_fits, opened};
use crate::general::world;

const COUNTRY: u64 = 1;
const CITY: u64 = 3;
const STREET: u64 = 10;

// 00.00. the level opened is the deepest the base holds: in a base without streets the city is
// listed counting nothing and opens its address, drawn inside the country
#[test]
#[ignore]
fn _00_00_the_deepest_level_of_a_base_without_streets_is_the_one_opened() {
  let w = world();
  let dir = resolved_at(
    w,
    "tui_deepest_is_a_city",
    &[
      area(COUNTRY, 2, "Country", [-10.0, -10.0], [20.0, 10.0]),
      area(CITY, 8, "City", [-2.0, -2.0], [5.0, 5.0]),
    ],
  );

  let listed = w.geolite_in(&dir, &["tui", "Country"]);
  assert_eq!(listed.status, 0, "stderr: {}", listed.stderr);
  assert_eq!(
    listed
      .stdout
      .lines()
      .last()
      .map(|line| line.split_whitespace().collect::<Vec<_>>()),
    Some(vec!["city", "City"]),
    "the deepest level counts nothing:\n{}",
    listed.stdout
  );

  let leaf = opened(w, &dir, "Country/City");
  assert_eq!(
    leaf.names(),
    ["name", "label", "point", "osm relation", "id", "levels"]
  );
  assert_eq!(leaf.field("name"), Some("City"));
  assert_eq!(leaf.field("label"), Some("City, Country"));
  assert_eq!(leaf.field("point"), Some("1.50000, 1.50000"));
  assert_eq!(leaf.field("osm relation"), Some("3"));
  assert_eq!(leaf.field("levels"), Some("Country"));
  assert_the_map_fits(&leaf);
  assert!(leaf.outline_glyphs() > 0, "the country is drawn");
}

// 00.01. the map is framed on the area the place was opened from and keeps the shape of the
// ground: a square city is drawn as wide as it is tall, a cell being twice as tall as it is wide,
// and a street along its south is drawn flat in the lower half of it
#[test]
#[ignore]
fn _00_01_the_map_draws_the_street_inside_the_area_it_was_opened_from() {
  let w = world();
  let dir = resolved_at(
    w,
    "tui_street_inside_a_city",
    &[
      area(COUNTRY, 2, "Country", [-10.0, -10.0], [20.0, 10.0]),
      area(CITY, 8, "City", [-2.0, -2.0], [5.0, 5.0]),
      street(STREET, "Street", &[[-1.0, 0.0], [4.0, 0.0]]),
    ],
  );

  let leaf = opened(w, &dir, "Country/City/Street");

  assert_eq!(leaf.field("osm way"), Some("10"));
  assert_eq!(leaf.field("levels"), Some("Country / City"));
  assert_the_map_fits(&leaf);
  let ((top, bottom), (left, right)) = leaf.shape_box().expect("the street is drawn");
  assert_eq!(top, bottom, "a street along a parallel is drawn on one row");
  assert!(
    top > MAP_ROWS / 2,
    "the street runs south of the middle of the city, row {top}"
  );
  let drawn = right - left + 1;
  let city = leaf
    .map
    .iter()
    .map(|line| line.trim_start().chars().count())
    .max()
    .unwrap_or(0);
  assert!(
    (city as f64 / (MAP_ROWS as f64 * 2.0) - 1.0).abs() < 0.2,
    "a square of {city} columns over {MAP_ROWS} rows is not a square"
  );
  assert!(
    (drawn as f64 / city as f64 - 5.0 / 7.0).abs() < 0.1,
    "the street takes five of the seven degrees of the city: {drawn} of {city} columns"
  );
  assert!(city < MAP_COLUMNS);
}

// 00.02. a place opened from the roots has no area around it: the map is framed on the place
// itself and the address names no level above it
#[test]
#[ignore]
fn _00_02_a_place_with_no_area_above_it_is_framed_on_itself() {
  let w = world();
  let dir = resolved_at(
    w,
    "tui_street_at_the_roots",
    &[street(STREET, "Street", &[[-1.0, 0.0], [4.0, 0.0]])],
  );

  let leaf = opened(w, &dir, "Street");

  assert_eq!(leaf.names(), ["name", "label", "point", "osm way", "id"]);
  assert_eq!(leaf.field("label"), Some("Street"));
  assert_the_map_fits(&leaf);
  assert_eq!(leaf.outline_glyphs(), 0, "nothing is around it");
  let (_, (left, right)) = leaf.shape_box().expect("the street is drawn");
  assert!(
    right - left + 1 > MAP_COLUMNS * 8 / 10,
    "the street takes the width of the map, less the margins: {left}..{right}"
  );
}
