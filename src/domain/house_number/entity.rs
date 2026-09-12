use geo::Point;

use super::strategy::link_strategy;
use super::value::house_number;
use crate::domain::admin_level::admin_level_id;

pub struct house_number_link {
  pub node_id: u64,
  pub street_id: admin_level_id,
  pub number: house_number,
  pub point: Point<f64>,
  pub strategy: link_strategy,
}
