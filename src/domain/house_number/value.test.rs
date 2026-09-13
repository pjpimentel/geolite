use super::{house_number, house_number_shape};
use crate::domain::house_number::house_number_policy;

use crate::domain::house_number::fixtures::{compound_policy, dropping_policy, simple_policy};

fn stored(raw: &str) -> Option<String> {
  house_number::normalize(raw, &simple_policy()).map(|n| n.stored_form().to_string())
}

#[test]
fn _00_pure_number_is_unchanged() {
  assert_eq!(stored("100").as_deref(), Some("100"));
}

#[test]
fn _01_leading_and_trailing_whitespace_is_trimmed() {
  assert_eq!(stored("  100  ").as_deref(), Some("100"));
}

#[test]
fn _02_space_separated_suffix_is_canonicalized() {
  assert_eq!(stored("12 a").as_deref(), Some("12A"));
}

#[test]
fn _03_hyphen_separated_suffix_is_canonicalized() {
  assert_eq!(stored("12-a").as_deref(), Some("12A"));
}

#[test]
fn _04_attached_suffix_is_canonicalized() {
  assert_eq!(stored("12a").as_deref(), Some("12A"));
}

#[test]
fn _05_already_uppercase_suffix_is_preserved() {
  assert_eq!(stored("12A").as_deref(), Some("12A"));
}

#[test]
fn _06_numeric_range_is_unchanged() {
  assert_eq!(stored("12-14").as_deref(), Some("12-14"));
}

#[test]
fn _07_non_house_number_strings_are_unchanged() {
  assert_eq!(stored("Lote 5").as_deref(), Some("Lote 5"));
  assert_eq!(stored("Fundos").as_deref(), Some("Fundos"));
}

#[test]
fn _08_empty_and_blank_values_are_discarded() {
  assert_eq!(stored(""), None);
  assert_eq!(stored("   "), None);
}

#[test]
fn _09_drop_values_are_discarded_case_insensitively_after_trim() {
  let policy = dropping_policy();
  for raw in ["s/n", "S/N", "  s/n  ", "Sn"] {
    assert_eq!(house_number::normalize(raw, &policy), None, "raw={raw}");
  }
}

#[test]
fn _10_drop_values_are_kept_when_drop_list_is_empty() {
  assert_eq!(stored("s/n").as_deref(), Some("s/n"));
}

#[test]
fn _11_compound_values_are_stored_untouched() {
  assert_eq!(stored("82-52").as_deref(), Some("82-52"));
  assert_eq!(stored("25B-48").as_deref(), Some("25B-48"));
  assert_eq!(stored("16i56").as_deref(), Some("16i56"));
}

fn recognized(token: &str, policy: &house_number_policy) -> Option<String> {
  house_number::recognize(token, policy).map(|n| n.stored_form().to_string())
}

#[test]
fn _12_plain_numbers_are_recognized() {
  let policy = simple_policy();
  for token in ["1", "35", "100", "12345"] {
    assert_eq!(recognized(token, &policy).as_deref(), Some(token));
  }
}

#[test]
fn _13_one_trailing_letter_is_recognized() {
  let policy = simple_policy();
  assert_eq!(recognized("123a", &policy).as_deref(), Some("123a"));
  assert_eq!(recognized("123A", &policy).as_deref(), Some("123A"));
}

#[test]
fn _14_a_single_trailing_comma_is_tolerated() {
  assert_eq!(recognized("35,", &simple_policy()).as_deref(), Some("35"));
}

#[test]
fn _15_postcodes_are_not_house_numbers() {
  let policy = simple_policy();
  assert_eq!(recognized("01310100", &policy), None);
  assert_eq!(recognized("01310-100", &policy), None);
}

#[test]
fn _16_more_than_one_trailing_character_is_rejected() {
  let policy = simple_policy();
  for token in ["12ab", "12-", "12-14", "s/n", "", "abc"] {
    assert_eq!(recognized(token, &policy), None, "token={token}");
  }
}

#[test]
fn _17_compound_forms_are_recognized_only_where_the_policy_allows() {
  let simple = simple_policy();
  let compound = compound_policy();
  for token in ["82-52", "25B-48", "16i56"] {
    assert_eq!(recognized(token, &simple), None, "token={token}");
    assert_eq!(recognized(token, &compound).as_deref(), Some(token));
  }
}

#[test]
fn _18_hash_prefix_is_stripped_only_where_the_policy_allows() {
  assert_eq!(recognized("#82", &simple_policy()), None);
  assert_eq!(recognized("#82", &compound_policy()).as_deref(), Some("82"));
  assert_eq!(
    recognized("#52-48", &compound_policy()).as_deref(),
    Some("52-48")
  );
}

#[test]
fn _19_a_hyphenated_postcode_is_not_a_compound_number() {
  assert_eq!(recognized("01310-100", &compound_policy()), None);
}

#[test]
fn _20_stored_and_typed_forms_of_the_same_number_compare_equal() {
  let policy = compound_policy();
  let pairs = [
    ("100", "100"),
    ("12 a", "12A"),
    ("12-a", "12a"),
    ("82-52", "82-52"),
    ("82-52", "#82-52"),
    ("25B-48", "25b-48"),
    ("16i56", "16I56"),
  ];
  for (written, typed) in pairs {
    let stored = house_number::normalize(written, &policy).expect("stored");
    let asked = house_number::recognize(typed, &policy).expect("typed");
    assert_eq!(stored, asked, "written={written} typed={typed}");
  }
}

#[test]
fn _21_different_numbers_do_not_compare_equal() {
  let policy = compound_policy();
  let a = house_number::normalize("82-52", &policy).expect("a");
  let b = house_number::recognize("52", &policy).expect("b");
  assert_ne!(a, b, "the tail of a compound is not the number itself");
}

#[test]
fn _22_compound_separators_normalize_to_one_key() {
  let policy = compound_policy();
  let key = |raw: &str| {
    house_number::normalize(raw, &policy)
      .expect("normalized")
      .comparison_key()
      .to_string()
  };
  assert_eq!(key("16i56"), "16I-56");
  assert_eq!(key("25b-48"), "25B-48");
  assert_eq!(key("82-52"), "82-52");
}

#[test]
fn _23_shape_reflects_the_written_form() {
  let policy = simple_policy();
  let shape = |raw: &str| house_number::normalize(raw, &policy).expect("normalized").shape();
  assert_eq!(shape("100"), house_number_shape::simple);
  assert_eq!(shape("12a"), house_number_shape::suffixed);
  assert_eq!(shape("82-52"), house_number_shape::compound);
  assert_eq!(shape("Lote 5"), house_number_shape::free);
}

#[test]
fn _24_leading_value_reads_the_digits_that_open_the_number() {
  let policy = simple_policy();
  let value = |raw: &str| {
    house_number::normalize(raw, &policy)
      .expect("normalized")
      .leading_value()
  };
  assert_eq!(value("123"), Some(123));
  assert_eq!(value("123a"), Some(123));
  assert_eq!(value("82-52"), Some(82));
  assert_eq!(value("Lote 5"), None);
}
