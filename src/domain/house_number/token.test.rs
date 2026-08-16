use super::*;

use crate::domain::house_number::policy::{COMPOUND_SHAPES, SIMPLE_SHAPES};

fn simple_policy() -> house_number_policy {
  house_number_policy {
    number_tags: &["addr:housenumber"],
    street_tags: &["addr:street"],
    drop_values: &[],
    max_digits: 5,
    shapes: SIMPLE_SHAPES,
    allow_hash_prefix: false,
  }
}

fn compound_policy() -> house_number_policy {
  house_number_policy {
    shapes: COMPOUND_SHAPES,
    allow_hash_prefix: true,
    ..simple_policy()
  }
}

fn found(query: &str, street_name: &str, policy: &house_number_policy) -> Option<String> {
  first_house_number(query, street_name, policy).map(|n| n.stored_form().to_string())
}

#[test]
fn _00_a_trailing_number_is_the_house_number() {
  assert_eq!(
    found("rua oscar freire 100", "rua oscar freire", &simple_policy()).as_deref(),
    Some("100")
  );
}

#[test]
fn _01_a_leading_number_is_the_house_number() {
  assert_eq!(
    found("100 rua oscar freire", "rua oscar freire", &simple_policy()).as_deref(),
    Some("100")
  );
}

#[test]
fn _02_a_number_in_the_middle_is_the_house_number() {
  assert_eq!(
    found(
      "rua castro alves 35, embare, santos",
      "rua castro alves",
      &simple_policy()
    )
    .as_deref(),
    Some("35")
  );
}

#[test]
fn _03_numbers_belonging_to_the_street_name_are_passed_over() {
  assert_eq!(
    found(
      "rua 25 de marco de 2024 100",
      "rua 25 de marco de 2024",
      &simple_policy()
    )
    .as_deref(),
    Some("100")
  );
}

#[test]
fn _04_a_query_that_is_only_the_street_name_has_no_house_number() {
  assert_eq!(
    found("rua 25 de marco", "rua 25 de marco", &simple_policy()),
    None
  );
}

#[test]
fn _05_only_the_first_remaining_number_is_taken() {
  assert_eq!(
    found("rua oscar freire 50 200", "rua oscar freire", &simple_policy()).as_deref(),
    Some("50")
  );
}

#[test]
fn _06_a_postcode_is_never_read_as_a_house_number() {
  let policy = simple_policy();
  for query in ["01310-100", "01310100"] {
    assert_eq!(found(query, "av paulista", &policy), None, "query={query}");
    assert!(!has_house_number(query, &policy), "query={query}");
  }
}

#[test]
fn _07_a_hash_attached_to_the_number_introduces_it() {
  assert_eq!(
    found("calle 82 #52-48", "calle 82", &compound_policy()).as_deref(),
    Some("52-48")
  );
}

#[test]
fn _08_a_hash_standing_alone_introduces_the_next_token() {
  assert_eq!(
    found("calle 82 # 52-48", "calle 82", &compound_policy()).as_deref(),
    Some("52-48")
  );
}

#[test]
fn _09_a_compound_number_is_invisible_where_the_policy_does_not_allow_it() {
  assert_eq!(found("rua x 12-14", "rua x", &simple_policy()), None);
  assert_eq!(
    found("rua x 12-14", "rua x", &compound_policy()).as_deref(),
    Some("12-14")
  );
}

#[test]
fn _10_has_house_number_answers_before_any_street_is_known() {
  let policy = simple_policy();
  assert!(has_house_number("rua x 100", &policy));
  assert!(!has_house_number("rua x", &policy));
}
