use super::{is_valid_key, json_path_of};

#[test]
fn _00_always_quotes_the_key_in_the_json_path() {
  assert_eq!(json_path_of("name"), "$.tags.\"name\"");
  assert_eq!(json_path_of("addr:postcode"), "$.tags.\"addr:postcode\"");
  assert_eq!(json_path_of("name:pt"), "$.tags.\"name:pt\"");
}

#[test]
fn _01_quotes_keys_a_bare_path_would_misread() {
  assert_eq!(json_path_of("a.b"), "$.tags.\"a.b\"");
  assert_eq!(json_path_of("c[1]"), "$.tags.\"c[1]\"");
}

#[test]
fn _02_rejects_keys_holding_a_dot_or_a_bracket() {
  assert!(!is_valid_key("a.b"));
  assert!(!is_valid_key("c[1]"));
  assert!(!is_valid_key("with space"));
  assert!(!is_valid_key(""));
}

#[test]
fn _03_accepts_the_shapes_osm_uses() {
  assert!(is_valid_key("name"));
  assert!(is_valid_key("name:pt"));
  assert!(is_valid_key("ISO3166-1:alpha2"));
  assert!(is_valid_key("addr_street"));
  assert!(is_valid_key("iso3166"));
}
