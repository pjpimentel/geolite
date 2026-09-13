use super::query_input;

#[test]
fn _00_valid_coordinates_parse_as_coordinates() {
  assert_eq!(
    query_input::parse("-23.5505,-46.6333"),
    query_input::coordinates {
      latitude: -23.5505,
      longitude: -46.6333
    }
  );
}

#[test]
fn _01_surrounding_and_inner_whitespace_is_trimmed() {
  assert_eq!(
    query_input::parse("  -23.5505 , -46.6333  "),
    query_input::coordinates {
      latitude: -23.5505,
      longitude: -46.6333
    }
  );
}

#[test]
fn _02_latitude_out_of_range_is_text() {
  assert_eq!(query_input::parse("91.0,0.0"), query_input::text);
}

#[test]
fn _03_longitude_out_of_range_is_text() {
  assert_eq!(query_input::parse("0.0,181.0"), query_input::text);
}

#[test]
fn _04_empty_string_is_text() {
  assert_eq!(query_input::parse(""), query_input::text);
}

#[test]
fn _05_plain_text_is_text() {
  assert_eq!(query_input::parse("rua oscar freire"), query_input::text);
}

#[test]
fn _06_single_number_without_comma_is_text() {
  assert_eq!(query_input::parse("-23.5505"), query_input::text);
}
