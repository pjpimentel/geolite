use geo::Point;

use super::strategy::link_strategy;
use super::value::house_number;
use crate::domain::admin_level::admin_level_id;

// a door number that has been placed on a street — the `house_numbers` table: which node it came
// from, which street owns it, and where on that street it sits.
pub struct house_number_link {
  pub node_id: u64,
  pub street_id: admin_level_id,
  pub number: house_number,
  // the node projected onto the street geometry, not the node's own position.
  pub point: Point<f64>,
  pub strategy: link_strategy,
}
