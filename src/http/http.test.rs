use super::{parse_bounding_wkt, parse_last_admin_levels, query_param, url_decode};

#[test]
fn _01_00_pure_ascii_string_returns_unchanged() {
  assert_eq!(url_decode("hello-world_123"), "hello-world_123");
}

#[test]
fn _01_01_plus_is_converted_to_space() {
  assert_eq!(url_decode("a+b+c"), "a b c");
}

#[test]
fn _01_02_percent_20_is_converted_to_space() {
  assert_eq!(url_decode("a%20b"), "a b");
}

#[test]
fn _01_03_multibyte_sequence_decodes_to_char() {
  assert_eq!(url_decode("%C3%A3"), "ã");
}

#[test]
fn _01_04_invalid_percent_escape_does_not_panic() {
  assert_eq!(url_decode("%ZZ"), "%ZZ");
  assert_eq!(url_decode("%2"), "%2");
  assert_eq!(url_decode("%"), "%");
}

#[test]
fn _01_05_empty_string_returns_empty() {
  assert_eq!(url_decode(""), "");
}

#[test]
fn _02_00_present_key_returns_decoded_value() {
  assert_eq!(query_param("q=hello", "q"), Some("hello".to_string()));
}

#[test]
fn _02_01_absent_key_returns_none() {
  assert_eq!(query_param("a=1&b=2", "c"), None);
}

#[test]
fn _02_02_multiple_params_returns_value_of_matching_key() {
  assert_eq!(query_param("a=1&b=2&c=3", "b"), Some("2".to_string()));
}

#[test]
fn _02_03_percent_20_in_value_is_decoded() {
  assert_eq!(query_param("q=a%20b", "q"), Some("a b".to_string()));
}

#[test]
fn _02_04_empty_query_string_returns_none() {
  assert_eq!(query_param("", "q"), None);
}

#[test]
fn _03_00_valid_polygon_returns_geometry_with_envelope() {
  let b =
    parse_bounding_wkt("POLYGON((-47.0 -24.0, -46.0 -24.0, -46.0 -23.0, -47.0 -23.0, -47.0 -24.0))")
      .unwrap();
  assert_eq!(b.envelope.min_lon, -47.0);
  assert_eq!(b.envelope.max_lon, -46.0);
  assert_eq!(b.envelope.min_lat, -24.0);
  assert_eq!(b.envelope.max_lat, -23.0);
}

#[test]
fn _03_01_multipolygon_is_accepted() {
  let b = parse_bounding_wkt(
    "MULTIPOLYGON(((0 0, 1 0, 1 1, 0 1, 0 0)), ((10 10, 11 10, 11 11, 10 11, 10 10)))",
  )
  .unwrap();
  assert_eq!(b.envelope.min_lon, 0.0);
  assert_eq!(b.envelope.max_lon, 11.0);
  assert_eq!(b.envelope.min_lat, 0.0);
  assert_eq!(b.envelope.max_lat, 11.0);
}

#[test]
fn _03_02_invalid_wkt_returns_error() {
  assert!(parse_bounding_wkt("not a wkt").is_err());
  assert!(parse_bounding_wkt("POLYGON((0 0, 1 1").is_err());
}

#[test]
fn _03_03_non_area_geometry_returns_error() {
  assert!(parse_bounding_wkt("POINT(0 0)").is_err());
  assert!(parse_bounding_wkt("LINESTRING(0 0, 1 1)").is_err());
}

#[test]
fn _03_04_contains_inner_point_and_excludes_outer_point() {
  let b = parse_bounding_wkt("POLYGON((-1 -1, 1 -1, 1 1, -1 1, -1 -1))").unwrap();
  assert!(b.contains(0.0, 0.0));
  assert!(!b.contains(5.0, 5.0));
}

#[test]
fn _04_00_single_level_parses() {
  assert_eq!(parse_last_admin_levels("10").unwrap(), vec![10]);
}

#[test]
fn _04_01_multiple_levels_parse() {
  assert_eq!(parse_last_admin_levels("8,10,12").unwrap(), vec![8, 10, 12]);
}

#[test]
fn _04_02_empty_value_errors() {
  assert!(parse_last_admin_levels("").is_err());
  assert!(parse_last_admin_levels(" , ").is_err());
}

#[test]
fn _04_03_non_numeric_level_errors() {
  assert!(parse_last_admin_levels("abc").is_err());
  assert!(parse_last_admin_levels("8,abc").is_err());
}

#[test]
fn _04_04_level_above_u8_range_errors() {
  assert!(parse_last_admin_levels("300").is_err());
}

#[test]
fn _04_05_whitespace_around_values_is_tolerated() {
  assert_eq!(parse_last_admin_levels(" 8 , 10 ").unwrap(), vec![8, 10]);
}
