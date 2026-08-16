use super::*;

#[test]
fn _00_maps_each_variant_to_its_osm_literal() {
  assert_eq!(osm_tag::addr_housenumber.key(), "addr:housenumber");
  assert_eq!(osm_tag::addr_postcode.key(), "addr:postcode");
  assert_eq!(osm_tag::addr_street.key(), "addr:street");
  assert_eq!(osm_tag::iso3166_1.key(), "ISO3166-1");
  assert_eq!(osm_tag::iso3166_1_alpha2.key(), "ISO3166-1:alpha2");
  assert_eq!(osm_tag::name.key(), "name");
}

#[test]
fn _01_always_quotes_the_key_in_the_json_path() {
  assert_eq!(osm_tag::name.json_path(), "$.tags.\"name\"");
  assert_eq!(
    osm_tag::addr_postcode.json_path(),
    "$.tags.\"addr:postcode\""
  );
  assert_eq!(json_path_of("name:pt"), "$.tags.\"name:pt\"");
}

#[test]
fn _02_quotes_keys_a_bare_path_would_misread() {
  assert_eq!(json_path_of("a.b"), "$.tags.\"a.b\"");
  assert_eq!(json_path_of("c[1]"), "$.tags.\"c[1]\"");
}

#[test]
fn _03_rejects_keys_holding_a_dot_or_a_bracket() {
  assert!(!is_valid_key("a.b"));
  assert!(!is_valid_key("c[1]"));
  assert!(!is_valid_key("with space"));
  assert!(!is_valid_key(""));
}

#[test]
fn _04_accepts_the_shapes_osm_uses() {
  assert!(is_valid_key("name"));
  assert!(is_valid_key("name:pt"));
  assert!(is_valid_key("ISO3166-1:alpha2"));
  assert!(is_valid_key("addr_street"));
  assert!(is_valid_key("iso3166"));
}

#[test]
fn _05_orders_alias_groups_by_priority() {
  assert_eq!(POST_CODE, &[osm_tag::postal_code, osm_tag::addr_postcode]);
  assert_eq!(
    COUNTRY_ISO,
    &[osm_tag::iso3166_1, osm_tag::iso3166_1_alpha2]
  );
}
