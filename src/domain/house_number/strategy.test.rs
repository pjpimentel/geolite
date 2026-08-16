use super::*;

#[test]
fn _00_codes_are_pinned_to_the_stored_format() {
  assert_eq!(link_strategy::by_proximity.code(), 0);
  assert_eq!(link_strategy::by_name.code(), 1);
}

#[test]
fn _01_known_codes_round_trip() {
  for strategy in [link_strategy::by_proximity, link_strategy::by_name] {
    assert_eq!(link_strategy::from_code(strategy.code()), Some(strategy));
  }
}

#[test]
fn _02_unknown_codes_are_rejected() {
  assert_eq!(link_strategy::from_code(2), None);
  assert_eq!(link_strategy::from_code(255), None);
}
