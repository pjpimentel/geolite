use geo::{Geometry, HaversineDistance, Point};
use rusqlite::Connection;
use std::collections::HashMap;

use super::{house_number_match, query_house_number};
use crate::extract::admin_levels::osm_admin_level;

// house numbers are precise points; 50m is intentionally tighter than the 100m used for
// streets, which are lines with a broader snap area
const MATCH_MAX_DISTANCE_IN_METERS: f64 = 50.0;

// upper bound on the digit count of a house-number token. a brazilian postcode is 8
// digits, so capping at 5 keeps postcodes (and longer numeric ids) out of the parser.
const MAX_HOUSE_NUMBER_DIGITS: usize = 5;

// the house-number core of a token (digits + optional single letter), ignoring a single
// trailing comma. None when the token isn't house-number shaped.
fn house_number_core(token: &str) -> Option<&str> {
  let core = token.strip_suffix(',').unwrap_or(token);
  is_house_number_token(core).then_some(core)
}

fn is_house_number_token(token: &str) -> bool {
  if token.is_empty() {
    return false;
  }
  let bytes = token.as_bytes();
  let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
  if digits == 0 || digits > MAX_HOUSE_NUMBER_DIGITS {
    return false;
  }
  match bytes.len() - digits {
    0 => true,
    1 => bytes[digits].is_ascii_alphabetic(),
    _ => false,
  }
}

// "123a" -> 123, "s/n" -> None. parses only the leading run of ascii digits, used to
// order house numbers for interpolation.
fn parse_leading_number(s: &str) -> Option<u32> {
  let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
  digits.parse().ok()
}

fn point_of(wkb: Option<&crate::database::admin_levels::admin_geometry>) -> Option<Point<f64>> {
  match wkb.map(|g| g.geometry()) {
    Some(Geometry::Point(p)) => Some(*p),
    _ => None,
  }
}

// resolves a house number from the query text against each street match, keyed by value (not
// proximity). per street: the street's own name tokens are removed from the query (so "25" in
// "rua 25 de marco" is dropped) and the FIRST remaining numeric occurrence is taken as the
// house number, then resolved as exact, else interpolated (bracketing), else absent. on
// exact/interpolated the match coordinate is moved to the house-number point and the number is
// appended as an admin_level (level 30), re-rendering friendly_name. a street with no remaining
// numeric token keeps house_number None.
pub fn enrich_house_number_from_query(
  conn: &Connection,
  query: &str,
  matches: &mut [super::query_match],
  friendly_name_format: Option<&str>,
) {
  let query_tokens: Vec<&str> = query.split_whitespace().collect();
  if !query_tokens.iter().any(|t| house_number_core(t).is_some()) {
    return;
  }

  let admin_level_ids: Vec<i64> = matches.iter().filter_map(|m| m.admin_level_id).collect();
  if admin_level_ids.is_empty() {
    return;
  }

  let raw_hns = crate::database::house_numbers::by_admin_level_ids(conn, &admin_level_ids);
  let mut hns_by_admin_level: HashMap<i64, Vec<crate::database::house_numbers::hn_for_street>> =
    HashMap::new();
  for hn in raw_hns {
    hns_by_admin_level.entry(hn.admin_level_id).or_default().push(hn);
  }

  for m in matches.iter_mut() {
    let admin_level_id = match m.admin_level_id {
      Some(id) => id,
      None => continue,
    };

    let street_name = street_name_of(m);
    let number = match first_house_number(&query_tokens, &street_name) {
      Some(n) => n,
      None => continue,
    };

    let resolved = hns_by_admin_level.get(&admin_level_id).and_then(|hns| {
      exact_point(hns, &number.trim().to_lowercase())
        .map(|pt| (house_number_match::exact, pt))
        .or_else(|| {
          parse_leading_number(&number)
            .and_then(|t| interpolate_point(hns, t))
            .map(|pt| (house_number_match::interpolated, pt))
        })
    });

    let kind = match resolved {
      Some((kind, pt)) => {
        m.latitude = super::round5(pt.y());
        m.longitude = super::round5(pt.x());
        m.admin_levels.push(super::admin_level {
          level: osm_admin_level::house_numbers as u8,
          name: number.clone(),
          osm_relation_id: None,
          osm_way_id: None,
          wkt: None,
        });
        m.friendly_name = match friendly_name_format {
          Some(fmt) => super::render_friendly_name(fmt, &m.admin_levels),
          None => super::default_friendly_name(&m.admin_levels),
        };
        // nudge similarity so a match with the house number resolved outranks the bare street
        if let Some(s) = m.similarity {
          m.similarity = Some(super::round5(s as f64 + 0.01) as f32);
        }
        kind
      }
      None => house_number_match::absent,
    };

    m.house_number = Some(query_house_number { number, kind });
  }
}

// the first house-number-shaped token in the query that isn't part of the street's own name.
fn first_house_number(query_tokens: &[&str], street_name: &str) -> Option<String> {
  query_tokens
    .iter()
    .copied()
    .filter(|t| !name_contains_token(street_name, t))
    .find_map(|t| house_number_core(t).map(str::to_string))
}

fn street_name_of(m: &super::query_match) -> String {
  let street = osm_admin_level::street as u8;
  m.admin_levels
    .iter()
    .find(|a| a.level == street)
    .map(|a| a.name.clone())
    .unwrap_or_default()
}

fn name_contains_token(name: &str, number: &str) -> bool {
  name
    .split(|c: char| !c.is_alphanumeric())
    .any(|t| t.eq_ignore_ascii_case(number))
}

fn exact_point(
  hns: &[crate::database::house_numbers::hn_for_street],
  wanted: &str,
) -> Option<Point<f64>> {
  hns
    .iter()
    .filter(|hn| hn.number.trim().to_lowercase() == wanted)
    .find_map(|hn| point_of(hn.wkb.as_ref()))
}

// bracketing: lerp between the nearest known number below and above the target
fn interpolate_point(
  hns: &[crate::database::house_numbers::hn_for_street],
  target: u32,
) -> Option<Point<f64>> {
  let mut below: Option<(u32, Point<f64>)> = None;
  let mut above: Option<(u32, Point<f64>)> = None;
  for hn in hns {
    let value = match parse_leading_number(&hn.number) {
      Some(v) => v,
      None => continue,
    };
    let pt = match point_of(hn.wkb.as_ref()) {
      Some(p) => p,
      None => continue,
    };
    if value < target {
      if below.is_none_or(|(b, _)| value > b) {
        below = Some((value, pt));
      }
    } else if value > target && above.is_none_or(|(a, _)| value < a) {
      above = Some((value, pt));
    }
  }

  match (below, above) {
    (Some((b, bp)), Some((a, ap))) => {
      let frac = (target - b) as f64 / (a - b) as f64;
      Some(Point::new(bp.x() + (ap.x() - bp.x()) * frac, bp.y() + (ap.y() - bp.y()) * frac))
    }
    _ => None,
  }
}

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

  let raw_hns = crate::database::house_numbers::by_admin_level_ids(conn, &admin_level_ids);
  let mut hns_by_admin_level: HashMap<i64, Vec<crate::database::house_numbers::hn_for_street>> =
    HashMap::new();
  for hn in raw_hns {
    hns_by_admin_level.entry(hn.admin_level_id).or_default().push(hn);
  }

  for m in matches.iter_mut() {
    let admin_level_id = match m.admin_level_id {
      Some(id) => id,
      None => continue,
    };

    let closest = hns_by_admin_level.get(&admin_level_id).and_then(|hns| {
      hns
        .iter()
        .filter_map(|hn| {
          let pt = match hn.wkb.as_ref().map(|g| g.geometry()) {
            Some(Geometry::Point(p)) => *p,
            _ => return None,
          };
          let d = input_pt.haversine_distance(&pt);
          if d <= MATCH_MAX_DISTANCE_IN_METERS {
            Some((hn, d))
          } else {
            None
          }
        })
        .min_by(|(_, d1), (_, d2)| d1.partial_cmp(d2).unwrap())
    });

    if let Some((hn, _)) = closest {
      m.admin_levels.push(super::admin_level {
        level: osm_admin_level::house_numbers as u8,
        name: hn.number.clone(),
        osm_relation_id: None,
        osm_way_id: None,
        wkt: None,
      });
      m.friendly_name = match friendly_name_format {
        Some(fmt) => super::render_friendly_name(fmt, &m.admin_levels),
        None => super::default_friendly_name(&m.admin_levels),
      };
    }
  }
}

#[cfg(test)]
#[path = "house_number.test.rs"]
mod tests;
