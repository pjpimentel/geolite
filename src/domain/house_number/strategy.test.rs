use super::link_strategy;

#[test]
fn _00_codes_are_pinned_to_the_stored_format() {
  assert_eq!(link_strategy::by_proximity.code(), 0);
  assert_eq!(link_strategy::by_name.code(), 1);
}
