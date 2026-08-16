use super::*;

use crate::domain::house_number::house_number_policy;
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

fn known(entries: &[(&str, f64, f64)]) -> Vec<(house_number, Point<f64>)> {
  entries
    .iter()
    .map(|(number, x, y)| (house_number::from_stored(number), Point::new(*x, *y)))
    .collect()
}

fn wanted(token: &str, policy: &house_number_policy) -> house_number {
  house_number::recognize(token, policy).expect("token must be recognized")
}

const STREET: &[(&str, f64, f64)] = &[
  ("100", 0.0, 0.0),
  ("200", 2.0, 2.0),
  ("300", 4.0, 4.0),
];

#[test]
fn _00_a_mapped_number_resolves_to_its_own_point() {
  let out = resolve(&wanted("200", &simple_policy()), &known(STREET));
  assert_eq!(out, house_number_resolution::exact(Point::new(2.0, 2.0)));
}

#[test]
fn _01_a_number_between_two_mapped_ones_is_interpolated() {
  let out = resolve(&wanted("150", &simple_policy()), &known(STREET));
  assert_eq!(
    out,
    house_number_resolution::interpolated(Point::new(1.0, 1.0))
  );
}

#[test]
fn _02_a_number_past_both_ends_cannot_be_placed() {
  let out = resolve(&wanted("400", &simple_policy()), &known(STREET));
  assert_eq!(out, house_number_resolution::absent);
}

#[test]
fn _03_a_street_with_no_mapped_numbers_cannot_place_anything() {
  let out = resolve(&wanted("100", &simple_policy()), &[]);
  assert_eq!(out, house_number_resolution::absent);
}

#[test]
fn _04_a_letter_suffix_matches_the_stored_canonical_form() {
  let street = known(&[("12A", 1.0, 1.0)]);
  let out = resolve(&wanted("12a", &simple_policy()), &street);
  assert_eq!(out, house_number_resolution::exact(Point::new(1.0, 1.0)));
}

#[test]
fn _05_a_compound_number_resolves_against_its_stored_value() {
  let policy = compound_policy();
  let street = known(&[("82-52", 1.0, 1.0), ("82-60", 2.0, 2.0)]);
  for token in ["82-52", "#82-52"] {
    let out = resolve(&wanted(token, &policy), &street);
    assert_eq!(
      out,
      house_number_resolution::exact(Point::new(1.0, 1.0)),
      "token={token}"
    );
  }
}

#[test]
fn _06_an_unmapped_compound_number_is_absent_rather_than_interpolated() {
  let policy = compound_policy();
  let street = known(&[("82-52", 1.0, 1.0), ("82-60", 2.0, 2.0)]);
  let out = resolve(&wanted("82-56", &policy), &street);
  assert_eq!(out, house_number_resolution::absent);
}

#[test]
fn _07_values_without_a_leading_number_are_ignored_when_interpolating() {
  let street = known(&[("100", 0.0, 0.0), ("s/n", 9.0, 9.0), ("300", 4.0, 4.0)]);
  let out = resolve(&wanted("200", &simple_policy()), &street);
  assert_eq!(
    out,
    house_number_resolution::interpolated(Point::new(2.0, 2.0))
  );
}
