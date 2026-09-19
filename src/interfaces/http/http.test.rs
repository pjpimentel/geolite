use super::{bind, parse_last_admin_levels, query_param, url_decode};
use crate::admin_level::level;

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
fn _03_00_single_level_parses() {
  assert_eq!(parse_last_admin_levels("10").unwrap(), vec![level::neighborhood]);
}

#[test]
fn _03_01_multiple_levels_parse() {
  assert_eq!(
    parse_last_admin_levels("8,10,12").unwrap(),
    vec![level::city, level::neighborhood, level::street]
  );
}

#[test]
fn _03_02_empty_value_errors() {
  assert!(parse_last_admin_levels("").is_err());
  assert!(parse_last_admin_levels(" , ").is_err());
}

#[test]
fn _03_03_non_numeric_level_errors() {
  assert!(parse_last_admin_levels("abc").is_err());
  assert!(parse_last_admin_levels("8,abc").is_err());
}

#[test]
fn _03_04_level_above_u8_range_errors() {
  assert!(parse_last_admin_levels("300").is_err());
}

#[test]
fn _03_05_whitespace_around_values_is_tolerated() {
  assert_eq!(
    parse_last_admin_levels(" 8 , 10 ").unwrap(),
    vec![level::city, level::neighborhood]
  );
}

#[test]
fn _03_06_a_level_outside_the_scale_errors_with_its_value() {
  assert_eq!(
    parse_last_admin_levels("8,11").unwrap_err(),
    "last_admin_levels: level 11 is not supported"
  );
  assert_eq!(
    parse_last_admin_levels("abc").unwrap_err(),
    "last_admin_levels: invalid level 'abc'"
  );
}

#[test]
fn _04_00_bind_on_port_zero_reports_the_port_the_os_chose() {
  let server = bind("127.0.0.1:0");

  assert_ne!(server.addr().port(), 0);
  assert_eq!(server.addr().ip(), std::net::IpAddr::from([127, 0, 0, 1]));
}
