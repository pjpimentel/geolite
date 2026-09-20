use geo::Point;
use rusqlite::Connection;
use std::collections::HashMap;

use super::entity::{house_number_match, query_house_number};
use crate::house_number::repository::numbers_by_street;
use crate::house_number::{
  house_number, house_number_policy, house_number_resolution, resolution, token,
};

pub(super) struct resolved_number {
  number: house_number,
  resolution: house_number_resolution,
}

impl resolved_number {
  pub(super) fn placed(&self) -> Option<(&str, Point<f64>)> {
    match self.resolution {
      house_number_resolution::exact(point) | house_number_resolution::interpolated(point) => {
        Some((self.number.stored_form(), point))
      }
      house_number_resolution::absent => None,
    }
  }

  pub(super) fn reported(&self) -> query_house_number {
    query_house_number {
      number: self.number.stored_form().to_string(),
      kind: match self.resolution {
        house_number_resolution::exact(_) => house_number_match::exact,
        house_number_resolution::interpolated(_) => house_number_match::interpolated,
        house_number_resolution::absent => house_number_match::absent,
      },
    }
  }
}

pub(super) fn from_query(
  conn: &Connection,
  query: &str,
  streets: &[(i64, &str)],
  policy: &house_number_policy,
) -> HashMap<i64, resolved_number> {
  if streets.is_empty() || !token::has_house_number(query, policy) {
    return HashMap::new();
  }
  let street_ids: Vec<i64> = streets.iter().map(|(id, _)| *id).collect();
  let by_street = numbers_by_street(conn, &street_ids);
  let no_numbers: Vec<(house_number, Point<f64>)> = Vec::new();

  streets
    .iter()
    .filter_map(|&(id, name)| {
      let number = token::first_house_number(query, name, policy)?;
      let known = by_street.get(&id).unwrap_or(&no_numbers);
      let resolution = resolution::resolve(&number, known);
      Some((id, resolved_number { number, resolution }))
    })
    .collect()
}

pub(super) fn nearest_to(
  conn: &Connection,
  input_pt: Point<f64>,
  street_ids: &[i64],
) -> HashMap<i64, house_number> {
  numbers_by_street(conn, street_ids)
    .into_iter()
    .filter_map(|(id, known)| {
      resolution::nearest(input_pt, &known)
        .cloned()
        .map(|number| (id, number))
    })
    .collect()
}
