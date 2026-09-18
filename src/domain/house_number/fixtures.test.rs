use geo::Point;

use super::entity::house_number_link;
use super::policy::{COMPOUND_SHAPES, house_number_policy};
use super::strategy::link_strategy;
use super::value::house_number;
use crate::domain::admin_level::admin_level_id;

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

pub(crate) fn link(
  node_id: u64,
  street_id: i64,
  number: &str,
  lon: f64,
  lat: f64,
) -> house_number_link {
  house_number_link {
    node_id,
    street_id: admin_level_id::from_raw(street_id as u64),
    number: house_number::from_stored(number),
    point: Point::new(lon, lat),
    strategy: link_strategy::by_proximity,
  }
}
