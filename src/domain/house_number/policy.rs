// a plain number or a number with one trailing letter — brazil, portugal and everywhere else
// that writes a door number as a single value.

use super::value::house_number_shape;

pub const SIMPLE_SHAPES: &[house_number_shape] =
  &[house_number_shape::simple, house_number_shape::suffixed];

// adds the compound pair of colombian nomenclature (`calle 82 # 52-48`), where the number names
// the nearest cross street and the distance to it.
pub const COMPOUND_SHAPES: &[house_number_shape] = &[
  house_number_shape::simple,
  house_number_shape::suffixed,
  house_number_shape::compound,
];

// how a region writes and reads door numbers. the same policy drives both sides of the concept:
// which osm tags carry the number, which values are not numbers at all, and which written forms
// are recognised when the number arrives inside a free-text query.
//
// recognition is deliberately opt-in per region: a hyphenated pair is a compound number in
// colombia (`calle 82 # 52-48`) and a range in brazil (`12-14`), and nothing in the string itself
// tells the two apart — only the region does.
#[derive(Clone, Copy)]
pub struct house_number_policy {
  pub number_tags: &'static [&'static str],
  pub street_tags: &'static [&'static str],
  pub drop_values: &'static [&'static str],
  // upper bound on the digit run of a number, so a postcode is never read as a door number
  // (a brazilian postcode is 8 digits).
  pub max_digits: u8,
  pub shapes: &'static [house_number_shape],
  // `#` introduces the door number in colombian addressing (`calle 82 # 52-48`).
  pub allow_hash_prefix: bool,
}
