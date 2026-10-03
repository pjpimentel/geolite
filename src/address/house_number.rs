use geo::Point;
use rusqlite::Connection;
use std::collections::HashMap;

use super::entity::query_house_number;
use crate::admin_level::repository::admin_area_row;
use crate::admin_level::spatial_index::nearest_street;
use crate::house_number::repository::numbers_by_street;
use crate::house_number::{
  house_number, house_number_policy, house_number_resolution, resolution, token,
};

pub(super) fn from_query(
  conn: &Connection,
  query: &str,
  streets: &[&admin_area_row],
  policy: &house_number_policy,
) -> HashMap<i64, house_number_resolution> {
  if streets.is_empty() || !token::has_house_number(query, policy) {
    return HashMap::new();
  }
  let street_ids: Vec<i64> = streets.iter().map(|street| street.id).collect();
  let by_street = numbers_by_street(conn, &street_ids);
  let no_numbers: Vec<(house_number, Point<f64>)> = Vec::new();

  streets
    .iter()
    .filter_map(|street| {
      let wanted = token::first_house_number(query, &street.name, policy)?;
      let known = by_street.get(&street.id).unwrap_or(&no_numbers);
      let geometry = street.wkb.as_ref()?.geometry();
      resolution::place(&wanted, known, geometry, policy).map(|placed| (street.id, placed))
    })
    .collect()
}

pub(super) fn at_point(
  conn: &Connection,
  input_pt: Point<f64>,
  candidates: &[nearest_street],
  policy: &house_number_policy,
) -> HashMap<i64, house_number_resolution> {
  let street_ids: Vec<i64> = candidates.iter().map(|c| c.id).collect();
  let by_street = numbers_by_street(conn, &street_ids);
  let no_numbers: Vec<(house_number, Point<f64>)> = Vec::new();

  candidates
    .iter()
    .filter_map(|c| {
      let known = by_street.get(&c.id).unwrap_or(&no_numbers);
      resolution::number_at(input_pt, c.closest_point, known, &c.geometry, policy)
        .map(|read| (c.id, read))
    })
    .collect()
}

pub(super) fn reported(resolution: &house_number_resolution) -> query_house_number {
  query_house_number {
    number: resolution.number.stored_form().to_string(),
    kind: resolution.scenario,
  }
}
