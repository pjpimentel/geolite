use super::round5;

#[test]
fn _00_rounds_positive_value_to_five_decimals() {
  assert_eq!(round5(1.123456789), 1.12346);
}

#[test]
fn _01_rounds_negative_value_to_five_decimals() {
  assert_eq!(round5(-1.123456789), -1.12346);
}

#[test]
fn _02_value_already_at_five_decimals_is_unchanged() {
  assert_eq!(round5(1.12346), 1.12346);
}
