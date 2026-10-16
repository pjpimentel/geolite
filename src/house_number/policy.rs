use super::value::house_number_shape;

pub const NUMBER_TAGS: &[&str] = &["addr:housenumber"];
pub const STREET_TAGS: &[&str] = &["addr:street"];

pub const SIMPLE_SHAPES: &[house_number_shape] =
  &[house_number_shape::simple, house_number_shape::suffixed];

pub const COMPOUND_SHAPES: &[house_number_shape] = &[
  house_number_shape::simple,
  house_number_shape::suffixed,
  house_number_shape::compound,
];

#[derive(Clone, Copy)]
pub struct house_number_policy {
  pub drop_values: &'static [&'static str],
  pub max_digits: u8,
  pub shapes: &'static [house_number_shape],
  pub allow_hash_prefix: bool,
  pub meters_per_number: f64,
}

pub const EVERY_SHAPE: house_number_policy = house_number_policy {
  shapes: COMPOUND_SHAPES,
  allow_hash_prefix: true,
  ..crate::presets::DEFAULT.house_numbers
};
