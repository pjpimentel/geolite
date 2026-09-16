use super::{apply_filters_and_truncate, bounding_geometry, parse_min_quality};
use crate::domain::address::fixtures::{admin_level_at, match_at};
use crate::domain::address::query_opts;
use crate::domain::admin_level::geometry::bounding_box;
use crate::domain::admin_level::level;

#[test]
fn _00_bounding_box_and_last_admin_levels_apply_as_and() {
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
    match_at(1, 0.0, 0.0, vec![admin_level_at(12, "a")]),
    match_at(2, 0.0, 0.0, vec![admin_level_at(8, "b")]),
    match_at(3, 5.0, 5.0, vec![admin_level_at(12, "c")]),
    match_at(4, 5.0, 5.0, vec![admin_level_at(8, "d")]),
  ];

  let opts = query_opts {
    bounding: Some(bounds),
    last_admin_levels: Some(vec![level::street]),
    ..Default::default()
  };
  apply_filters_and_truncate(&mut matches, &opts);

  let surviving: Vec<String> = matches.iter().map(|m| m.id.clone()).collect();
  assert_eq!(surviving, vec![crate::domain::address::entity::path_id(1, &[])]);
}

#[test]
fn _01_parse_min_quality_accepts_the_boundaries_and_a_midpoint() {
  assert_eq!(parse_min_quality("0.0"), Ok(0.0));
  assert_eq!(parse_min_quality(" 0.5 "), Ok(0.5));
  assert_eq!(parse_min_quality("1.0"), Ok(1.0));
}

#[test]
fn _02_parse_min_quality_rejects_a_non_number() {
  let err = parse_min_quality("abc").expect_err("a non-number must be rejected");
  assert_eq!(err, "quality: not a number: invalid float literal");
}

#[test]
fn _03_parse_min_quality_rejects_values_outside_the_range() {
  for input in ["-0.1", "1.1", "NaN"] {
    let err = parse_min_quality(input).expect_err("out of range must be rejected");
    assert!(
      err.starts_with("quality: must be between 0.0 and 1.0, got "),
      "unexpected error: {err}"
    );
  }
}
