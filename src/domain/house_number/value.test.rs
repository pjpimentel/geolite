use super::*;

const SIMPLE_SHAPES: &[house_number_shape] =
  &[house_number_shape::simple, house_number_shape::suffixed];

const COMPOUND_SHAPES: &[house_number_shape] = &[
  house_number_shape::simple,
  house_number_shape::suffixed,
  house_number_shape::compound,
];

const BR_DROPS: &[&str] = &["s/n", "sn", "s/nº", "s/no"];

// the default written forms: a plain number or a number with one trailing letter.
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

fn dropping_policy() -> house_number_policy {
  house_number_policy {
    drop_values: BR_DROPS,
    ..simple_policy()
  }
}

// the colombian written forms: everything above plus the compound pair and the `#` prefix.
fn compound_policy() -> house_number_policy {
  house_number_policy {
    shapes: COMPOUND_SHAPES,
    allow_hash_prefix: true,
    ..simple_policy()
  }
}

fn stored(raw: &str) -> Option<String> {
  house_number::normalize(raw, &simple_policy()).map(|n| n.stored_form().to_string())
}

/////////////////////////////////////////////////////////////////////////////////
// 00 — normalize: the ingestion rule, moved out of sql unchanged
/////////////////////////////////////////////////////////////////////////////////

#[test]
fn _00_00_pure_number_is_unchanged() {
  assert_eq!(stored("100").as_deref(), Some("100"));
}

#[test]
fn _00_01_leading_and_trailing_whitespace_is_trimmed() {
  assert_eq!(stored("  100  ").as_deref(), Some("100"));
}

#[test]
fn _00_02_space_separated_suffix_is_canonicalized() {
  assert_eq!(stored("12 a").as_deref(), Some("12A"));
}

#[test]
fn _00_03_hyphen_separated_suffix_is_canonicalized() {
  assert_eq!(stored("12-a").as_deref(), Some("12A"));
}

#[test]
fn _00_04_attached_suffix_is_canonicalized() {
  assert_eq!(stored("12a").as_deref(), Some("12A"));
}

#[test]
fn _00_05_already_uppercase_suffix_is_preserved() {
  assert_eq!(stored("12A").as_deref(), Some("12A"));
}

#[test]
fn _00_06_numeric_range_is_unchanged() {
  assert_eq!(stored("12-14").as_deref(), Some("12-14"));
}

#[test]
fn _00_07_non_house_number_strings_are_unchanged() {
  assert_eq!(stored("Lote 5").as_deref(), Some("Lote 5"));
  assert_eq!(stored("Fundos").as_deref(), Some("Fundos"));
}

#[test]
fn _00_08_empty_and_blank_values_are_discarded() {
  assert_eq!(stored(""), None);
  assert_eq!(stored("   "), None);
}

#[test]
fn _00_09_drop_values_are_discarded_case_insensitively_after_trim() {
  let policy = dropping_policy();
  for raw in ["s/n", "S/N", "  s/n  ", "Sn"] {
    assert_eq!(house_number::normalize(raw, &policy), None, "raw={raw}");
  }
}

#[test]
fn _00_10_drop_values_are_kept_when_drop_list_is_empty() {
  assert_eq!(stored("s/n").as_deref(), Some("s/n"));
}

#[test]
fn _00_11_compound_values_are_stored_untouched() {
  // colombian nomenclature already reached the database intact; normalisation must not move it.
  assert_eq!(stored("82-52").as_deref(), Some("82-52"));
  assert_eq!(stored("25B-48").as_deref(), Some("25B-48"));
  assert_eq!(stored("16i56").as_deref(), Some("16i56"));
}

/////////////////////////////////////////////////////////////////////////////////
// 01 — recognize: the query rule
/////////////////////////////////////////////////////////////////////////////////

fn recognized(token: &str, policy: &house_number_policy) -> Option<String> {
  house_number::recognize(token, policy).map(|n| n.stored_form().to_string())
}

#[test]
fn _01_00_plain_numbers_are_recognized() {
  let policy = simple_policy();
  for token in ["1", "35", "100", "12345"] {
    assert_eq!(recognized(token, &policy).as_deref(), Some(token));
  }
}

#[test]
fn _01_01_one_trailing_letter_is_recognized() {
  let policy = simple_policy();
  assert_eq!(recognized("123a", &policy).as_deref(), Some("123a"));
  assert_eq!(recognized("123A", &policy).as_deref(), Some("123A"));
}

#[test]
fn _01_02_a_single_trailing_comma_is_tolerated() {
  assert_eq!(recognized("35,", &simple_policy()).as_deref(), Some("35"));
}

#[test]
fn _01_03_postcodes_are_not_house_numbers() {
  let policy = simple_policy();
  // eight digits is a brazilian postcode, past the five-digit cap.
  assert_eq!(recognized("01310100", &policy), None);
  // the hyphenated form is not a compound number either, under a policy that has no compound.
  assert_eq!(recognized("01310-100", &policy), None);
}

#[test]
fn _01_04_more_than_one_trailing_character_is_rejected() {
  let policy = simple_policy();
  for token in ["12ab", "12-", "12-14", "s/n", "", "abc"] {
    assert_eq!(recognized(token, &policy), None, "token={token}");
  }
}

#[test]
fn _01_05_compound_forms_are_recognized_only_where_the_policy_allows() {
  let simple = simple_policy();
  let compound = compound_policy();
  for token in ["82-52", "25B-48", "16i56"] {
    assert_eq!(recognized(token, &simple), None, "token={token}");
    assert_eq!(recognized(token, &compound).as_deref(), Some(token));
  }
}

#[test]
fn _01_06_hash_prefix_is_stripped_only_where_the_policy_allows() {
  assert_eq!(recognized("#82", &simple_policy()), None);
  assert_eq!(recognized("#82", &compound_policy()).as_deref(), Some("82"));
  assert_eq!(
    recognized("#52-48", &compound_policy()).as_deref(),
    Some("52-48")
  );
}

#[test]
fn _01_07_a_hyphenated_postcode_is_not_a_compound_number() {
  // the leading zero of a postcode is what tells it apart from colombian nomenclature.
  assert_eq!(recognized("01310-100", &compound_policy()), None);
}

/////////////////////////////////////////////////////////////////////////////////
// 02 — the invariant that makes the two sides agree
/////////////////////////////////////////////////////////////////////////////////

// what is written by the ingestion side and what is typed by the user must land on the same
// comparison key. this is the property that was missing while the rule lived in two languages.
#[test]
fn _02_00_stored_and_typed_forms_of_the_same_number_compare_equal() {
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
fn _02_01_different_numbers_do_not_compare_equal() {
  let policy = compound_policy();
  let a = house_number::normalize("82-52", &policy).expect("a");
  let b = house_number::recognize("52", &policy).expect("b");
  assert_ne!(a, b, "the tail of a compound is not the number itself");
}

#[test]
fn _02_02_compound_separators_normalize_to_one_key() {
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

/////////////////////////////////////////////////////////////////////////////////
// 03 — shape and leading value
/////////////////////////////////////////////////////////////////////////////////

#[test]
fn _03_00_shape_reflects_the_written_form() {
  let policy = simple_policy();
  let shape = |raw: &str| house_number::normalize(raw, &policy).expect("normalized").shape();
  assert_eq!(shape("100"), house_number_shape::simple);
  assert_eq!(shape("12a"), house_number_shape::suffixed);
  assert_eq!(shape("82-52"), house_number_shape::compound);
  assert_eq!(shape("Lote 5"), house_number_shape::free);
}

#[test]
fn _03_01_leading_value_reads_the_digits_that_open_the_number() {
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
