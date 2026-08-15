use geo::Point;

use super::value::house_number;
use crate::domain::kernel::admin_area_id::admin_area_id;

// a door number that has been placed on a street: which node it came from, which street owns it,
// and where on that street it sits.
pub struct house_number_link {
  pub node_id: u64,
  pub street_id: admin_area_id,
  pub number: house_number,
  // the node projected onto the street geometry, not the node's own position.
  pub point: Point<f64>,
  pub strategy: link_strategy,
}

// how a house number node was attached to its street.
//
// `by_name` means the node's street tag matched the street's name; `by_proximity` means it was
// snapped to the nearest street geometry. the numeric codes are persisted in the `strategy` column
// of `house_numbers` and are part of the on-disk format, so they must not change.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum link_strategy {
  by_proximity,
  by_name,
}

impl link_strategy {
  pub fn code(self) -> u8 {
    match self {
      link_strategy::by_proximity => 0,
      link_strategy::by_name => 1,
    }
  }

  #[allow(dead_code)]
  pub fn from_code(code: u8) -> Option<Self> {
    match code {
      0 => Some(link_strategy::by_proximity),
      1 => Some(link_strategy::by_name),
      _ => None,
    }
  }
}

#[cfg(test)]
#[path = "link.test.rs"]
mod tests;
