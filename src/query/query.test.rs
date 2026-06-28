use super::*;

fn al(level: u8, name: &str) -> admin_level {
  admin_level {
    level,
    name: name.to_string(),
    osm_relation_id: None,
    osm_way_id: None,
    wkt: None,
  }
}

#[test]
fn _00_render_template_resolves_all_placeholders() {
  let admins = vec![al(2, "brasil"), al(8, "santos"), al(12, "rua x")];
  let out = render_friendly_name(
    "{admin_level_12_name}, {admin_level_8_name}, {admin_level_2_name}",
    &admins,
  );
  assert_eq!(out, "rua x, santos, brasil");
}

#[test]
fn _01_render_template_omits_missing_placeholder_and_collapses_separator() {
  let admins = vec![al(2, "brasil"), al(12, "rua x")];
  let out = render_friendly_name(
    "{admin_level_12_name}, {admin_level_8_name}, {admin_level_2_name}",
    &admins,
  );
  assert_eq!(out, "rua x, brasil");
}

#[test]
fn _02_validate_format_rejects_unknown_field() {
  let err = validate_friendly_name_format("{foo} {admin_level_12_name}").unwrap_err();
  assert!(err.contains("unknown field 'foo'"), "got: {err}");
}

#[test]
fn _03_render_template_trims_trailing_when_last_placeholder_missing() {
  let admins = vec![al(12, "rua x")];
  let out = render_friendly_name("{admin_level_12_name}, {admin_level_8_name}", &admins);
  assert_eq!(out, "rua x");
}

#[test]
fn _04_validate_format_rejects_non_numeric_admin_level() {
  let err = validate_friendly_name_format("{admin_level_x_name}").unwrap_err();
  assert!(err.contains("invalid admin level 'x'"), "got: {err}");
}

#[test]
fn _05_validate_format_rejects_out_of_range_admin_level() {
  let err = validate_friendly_name_format("{admin_level_999_name}").unwrap_err();
  assert!(err.contains("invalid admin level '999'"), "got: {err}");
}

#[test]
fn _06_validate_format_rejects_unterminated_placeholder() {
  let err = validate_friendly_name_format("{admin_level_2_name").unwrap_err();
  assert!(err.contains("unterminated placeholder"), "got: {err}");
}

#[test]
fn _07_validate_format_accepts_valid_template() {
  let format = "{admin_level_12_name}, {admin_level_2_name}";
  assert_eq!(validate_friendly_name_format(format).unwrap(), format);
}

#[test]
fn _08_validate_format_accepts_empty_template() {
  assert_eq!(validate_friendly_name_format("").unwrap(), "");
}

#[test]
fn _09_validate_format_accepts_house_number_alias() {
  assert_eq!(
    validate_friendly_name_format("{house_number}").unwrap(),
    "{house_number}"
  );
}

#[test]
fn _10_render_resolves_house_number_alias() {
  let admins = vec![al(12, "rua x"), al(30, "123")];
  let out = render_friendly_name("{admin_level_12_name} {house_number}", &admins);
  assert_eq!(out, "rua x 123");
}

#[test]
fn _11_valid_coordinates_return_some() {
  assert_eq!(
    try_parse_coordinates("-23.5505,-46.6333"),
    Some((-23.5505, -46.6333))
  );
}

#[test]
fn _12_surrounding_and_inner_whitespace_is_trimmed() {
  assert_eq!(
    try_parse_coordinates("  -23.5505 , -46.6333  "),
    Some((-23.5505, -46.6333))
  );
}

#[test]
fn _13_latitude_out_of_range_returns_none() {
  assert_eq!(try_parse_coordinates("91.0,0.0"), None);
}

#[test]
fn _14_longitude_out_of_range_returns_none() {
  assert_eq!(try_parse_coordinates("0.0,181.0"), None);
}

#[test]
fn _15_empty_string_returns_none() {
  assert_eq!(try_parse_coordinates(""), None);
}

#[test]
fn _16_plain_text_returns_none() {
  assert_eq!(try_parse_coordinates("rua oscar freire"), None);
}

#[test]
fn _17_single_number_without_comma_returns_none() {
  assert_eq!(try_parse_coordinates("-23.5505"), None);
}

#[test]
fn _18_rounds_positive_value_to_five_decimals() {
  assert_eq!(round5(1.123456789), 1.12346);
}

#[test]
fn _19_rounds_negative_value_to_five_decimals() {
  assert_eq!(round5(-1.123456789), -1.12346);
}

#[test]
fn _20_value_already_at_five_decimals_is_unchanged() {
  assert_eq!(round5(1.12346), 1.12346);
}

fn match_at(id: u64, lat: f64, lon: f64, admin_levels: Vec<admin_level>) -> query_match {
  query_match {
    admin_levels,
    latitude: lat,
    longitude: lon,
    coordinates_distance_in_meters: None,
    similarity: None,
    score: None,
    friendly_name: String::new(),
    attributes: query_match_attributes {
      country_iso_3166_1_alpha_2_code: None,
      post_code: None,
    },
    house_number: None,
    id,
    admin_level_id: None,
  }
}

#[test]
fn _21_bounding_box_and_last_admin_levels_apply_as_and() {
  let bounds = bounding_geometry::from_rect(bounding_box {
    min_lat: -1.0,
    max_lat: 1.0,
    min_lon: -1.0,
    max_lon: 1.0,
  });

  // a: in box  + leaf level 12 → survives both filters
  // b: in box  + leaf level 8  → dropped by the last_admin_levels filter
  // c: out box + leaf level 12 → dropped by the bounding_box filter
  // d: out box + leaf level 8  → dropped by both
  let mut matches = vec![
    match_at(1, 0.0, 0.0, vec![al(12, "a")]),
    match_at(2, 0.0, 0.0, vec![al(8, "b")]),
    match_at(3, 5.0, 5.0, vec![al(12, "c")]),
    match_at(4, 5.0, 5.0, vec![al(8, "d")]),
  ];

  apply_filters_and_truncate(&mut matches, None, Some(&bounds), Some(&[12]));

  let surviving: Vec<u64> = matches.iter().map(|m| m.id).collect();
  assert_eq!(surviving, vec![1]);
}
