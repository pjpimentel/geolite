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

#[test]
fn _03_the_path_id_names_the_path_and_not_the_area() {
  let root = super::path_id(5, &[]);
  let through_a = super::path_id(5, &[3, 1]);
  let through_b = super::path_id(5, &[4, 1]);

  assert_eq!(root, super::path_id(5, &[]), "the same path is the same id");
  assert_ne!(through_a, through_b, "two paths of one area are two ids");
  assert_ne!(through_a, root);
  assert_ne!(through_a, super::path_id(6, &[3, 1]), "two areas are two ids");
  assert_eq!(through_a.len(), 36, "a uuid in its hyphenated form");
}

#[test]
fn _04_the_path_id_is_stable_across_runs() {
  assert_eq!(
    super::path_id(1_458_411_426, &[8_565_765, 596_885, 596_409, 118_941]),
    "92d2fd8c-a4e1-5fa6-a3cc-43ee45c32b45"
  );
}
