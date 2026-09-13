use geo::{Geometry, HaversineDistance, Point};
use rusqlite::Connection;
use std::collections::HashMap;

use super::entity::{house_number_match, query_house_number, query_match, round5};
use crate::domain::admin_level::level;
use crate::domain::house_number::{
  house_number, house_number_policy, house_number_resolution, resolution, token,
};

// house numbers are precise points; 50m is intentionally tighter than the 100m used for
// streets, which are lines with a broader snap area
const MATCH_MAX_DISTANCE_IN_METERS: f64 = 50.0;

fn point_of(wkb: Option<&crate::domain::admin_level::geometry::admin_geometry>) -> Option<Point<f64>> {
  match wkb.map(|g| g.geometry()) {
    Some(Geometry::Point(p)) => Some(*p),
    _ => None,
  }
}

fn numbers_by_street(
  conn: &Connection,
  admin_level_ids: &[i64],
) -> HashMap<i64, Vec<(house_number, Point<f64>)>> {
  let mut by_street: HashMap<i64, Vec<(house_number, Point<f64>)>> = HashMap::new();
  for row in crate::domain::house_number::repository::by_admin_level_ids(conn, admin_level_ids) {
    if let Some(point) = point_of(row.wkb.as_ref()) {
      by_street
        .entry(row.admin_level_id)
        .or_default()
        .push((row.number, point));
    }
  }
  by_street
}

fn place_on_house_number(
  m: &mut query_match,
  point: Point<f64>,
  number: &house_number,
  friendly_name_format: Option<&str>,
) {
  m.latitude = round5(point.y());
  m.longitude = round5(point.x());
  m.append_house_number_level(number.stored_form(), friendly_name_format);
  // nudge similarity so a match with the house number resolved outranks the bare street
  if let Some(s) = m.similarity {
    m.similarity = Some(round5(s as f64 + 0.01) as f32);
  }
}

pub(super) fn enrich_house_number_from_query(
  conn: &Connection,
  query: &str,
  matches: &mut [query_match],
  friendly_name_format: Option<&str>,
  policy: &house_number_policy,
) {
  if !token::has_house_number(query, policy) {
    return;
  }

  let admin_level_ids: Vec<i64> = matches.iter().filter_map(|m| m.admin_level_id).collect();
  if admin_level_ids.is_empty() {
    return;
  }
  let by_street = numbers_by_street(conn, &admin_level_ids);
  let no_numbers: Vec<(house_number, Point<f64>)> = Vec::new();

  for m in matches.iter_mut() {
    let Some(admin_level_id) = m.admin_level_id else {
      continue;
    };
    let street_name = street_name_of(m);
    let Some(number) = token::first_house_number(query, &street_name, policy) else {
      continue;
    };

    let known = by_street.get(&admin_level_id).unwrap_or(&no_numbers);
    let kind = match resolution::resolve(&number, known) {
      house_number_resolution::exact(point) => {
        place_on_house_number(m, point, &number, friendly_name_format);
        house_number_match::exact
      }
      house_number_resolution::interpolated(point) => {
        place_on_house_number(m, point, &number, friendly_name_format);
        house_number_match::interpolated
      }
      house_number_resolution::absent => house_number_match::absent,
    };

    m.house_number = Some(query_house_number {
      number: number.stored_form().to_string(),
      kind,
    });
  }
}

fn street_name_of(m: &query_match) -> String {
  let street = level::street.value();
  m.admin_levels
    .iter()
    .find(|a| a.level == street)
    .map(|a| a.name.clone())
    .unwrap_or_default()
}

pub(super) fn enrich_house_numbers(
  conn: &Connection,
  input_pt: Point<f64>,
  matches: &mut [query_match],
  friendly_name_format: Option<&str>,
) {
  let admin_level_ids: Vec<i64> = matches.iter().filter_map(|m| m.admin_level_id).collect();
  if admin_level_ids.is_empty() {
    return;
  }
  let by_street = numbers_by_street(conn, &admin_level_ids);

  for m in matches.iter_mut() {
    let Some(admin_level_id) = m.admin_level_id else {
      continue;
    };

    let closest = by_street.get(&admin_level_id).and_then(|numbers| {
      numbers
        .iter()
        .filter_map(|(number, point)| {
          let distance = input_pt.haversine_distance(point);
          (distance <= MATCH_MAX_DISTANCE_IN_METERS).then_some((number, distance))
        })
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
    });

    if let Some((number, _)) = closest {
      let name = number.stored_form().to_string();
      m.append_house_number_level(&name, friendly_name_format);
    }
  }
}

#[cfg(test)]
#[path = "house_number.test.rs"]
mod tests;
