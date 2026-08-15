use geo::{Geometry, HaversineDistance, Point};
use rusqlite::Connection;
use std::collections::HashMap;

use super::{house_number_match, query_house_number};
use crate::domain::house_number::house_number;
use crate::domain::house_number::{house_number_policy, house_number_resolution, resolution, token};
use crate::extract::admin_levels::osm_admin_level;

// house numbers are precise points; 50m is intentionally tighter than the 100m used for
// streets, which are lines with a broader snap area
const MATCH_MAX_DISTANCE_IN_METERS: f64 = 50.0;

fn point_of(wkb: Option<&crate::database::admin_levels::admin_geometry>) -> Option<Point<f64>> {
  match wkb.map(|g| g.geometry()) {
    Some(Geometry::Point(p)) => Some(*p),
    _ => None,
  }
}

// loads the numbers mapped on each of the matched streets, keeping only those that carry a point.
// a number without a point cannot answer anything, and dropping it here is what lets the domain
// work on plain (number, point) pairs.
fn numbers_by_street(
  conn: &Connection,
  admin_level_ids: &[i64],
) -> HashMap<i64, Vec<(house_number, Point<f64>)>> {
  let mut by_street: HashMap<i64, Vec<(house_number, Point<f64>)>> = HashMap::new();
  for row in crate::database::house_numbers::by_admin_level_ids(conn, admin_level_ids) {
    if let Some(point) = point_of(row.wkb.as_ref()) {
      by_street
        .entry(row.admin_level_id)
        .or_default()
        .push((row.number, point));
    }
  }
  by_street
}

// moves the match onto the house number's point and appends the number as a level-30 admin level,
// re-rendering the friendly name over the enriched list.
fn place_on_house_number(
  m: &mut super::query_match,
  point: Point<f64>,
  number: &house_number,
  friendly_name_format: Option<&str>,
) {
  m.latitude = super::round5(point.y());
  m.longitude = super::round5(point.x());
  append_house_number_level(m, number.stored_form(), friendly_name_format);
  // nudge similarity so a match with the house number resolved outranks the bare street
  if let Some(s) = m.similarity {
    m.similarity = Some(super::round5(s as f64 + 0.01) as f32);
  }
}

fn append_house_number_level(
  m: &mut super::query_match,
  number: &str,
  friendly_name_format: Option<&str>,
) {
  m.admin_levels.push(super::admin_level {
    level: osm_admin_level::house_numbers as u8,
    name: number.to_string(),
    osm_relation_id: None,
    osm_way_id: None,
    wkt: None,
  });
  m.friendly_name = match friendly_name_format {
    Some(fmt) => super::render_friendly_name(fmt, &m.admin_levels),
    None => super::default_friendly_name(&m.admin_levels),
  };
}

// resolves the house number written in the query against each street match, by value rather than
// by proximity. everything about what a number *is* — which tokens read as one, which of the
// query's numbers belongs to the street's own name, and how a number is placed — lives in the
// house_number domain; this function only carries data between the database and it.
pub fn enrich_house_number_from_query(
  conn: &Connection,
  query: &str,
  matches: &mut [super::query_match],
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
    let admin_level_id = match m.admin_level_id {
      Some(id) => id,
      None => continue,
    };
    let street_name = street_name_of(m);
    let number = match token::first_house_number(query, &street_name, policy) {
      Some(number) => number,
      None => continue,
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

fn street_name_of(m: &super::query_match) -> String {
  let street = osm_admin_level::street as u8;
  m.admin_levels
    .iter()
    .find(|a| a.level == street)
    .map(|a| a.name.clone())
    .unwrap_or_default()
}

// the coordinate path: there is no number in the input, so the nearest mapped number wins —
// provided it is close enough to be the same address.
pub fn enrich_house_numbers(
  conn: &Connection,
  input_pt: Point<f64>,
  matches: &mut [super::query_match],
  friendly_name_format: Option<&str>,
) {
  let admin_level_ids: Vec<i64> = matches.iter().filter_map(|m| m.admin_level_id).collect();
  if admin_level_ids.is_empty() {
    return;
  }
  let by_street = numbers_by_street(conn, &admin_level_ids);

  for m in matches.iter_mut() {
    let admin_level_id = match m.admin_level_id {
      Some(id) => id,
      None => continue,
    };

    let closest = by_street.get(&admin_level_id).and_then(|numbers| {
      numbers
        .iter()
        .filter_map(|(number, point)| {
          let distance = input_pt.haversine_distance(point);
          if distance <= MATCH_MAX_DISTANCE_IN_METERS {
            Some((number, distance))
          } else {
            None
          }
        })
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
    });

    if let Some((number, _)) = closest {
      let name = number.stored_form().to_string();
      append_house_number_level(m, &name, friendly_name_format);
    }
  }
}

#[cfg(test)]
#[path = "house_number.test.rs"]
mod tests;
