use super::policy::{COMPOUND_SHAPES, house_number_policy};

pub(crate) const BR_DROPS: &[&str] = &["s/n", "sn", "s/nº", "s/no"];

pub(crate) fn simple_policy() -> house_number_policy {
  crate::presets::DEFAULT.house_numbers
}

pub(crate) fn dropping_policy() -> house_number_policy {
  house_number_policy {
    drop_values: BR_DROPS,
    ..simple_policy()
  }
}

pub(crate) fn compound_policy() -> house_number_policy {
  house_number_policy {
    shapes: COMPOUND_SHAPES,
    allow_hash_prefix: true,
    ..simple_policy()
  }
}
