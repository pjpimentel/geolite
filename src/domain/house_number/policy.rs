use super::value::house_number_shape;

pub const SIMPLE_SHAPES: &[house_number_shape] =
  &[house_number_shape::simple, house_number_shape::suffixed];

pub const COMPOUND_SHAPES: &[house_number_shape] = &[
  house_number_shape::simple,
  house_number_shape::suffixed,
  house_number_shape::compound,
];

#[derive(Clone, Copy)]
pub struct house_number_policy {
  pub number_tags: &'static [&'static str],
  pub street_tags: &'static [&'static str],
  pub drop_values: &'static [&'static str],
  pub max_digits: u8,
  pub shapes: &'static [house_number_shape],
  pub allow_hash_prefix: bool,
}
