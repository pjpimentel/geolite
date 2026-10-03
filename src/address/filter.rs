use super::entity::query_match;
use super::query_opts;
use crate::admin_level::level;
use crate::house_number::house_number_scenario;

pub(super) const MAX_RESULTS: u8 = 10;
const COORDINATE_QUALITY_REFERENCE_M: f64 = 100.0;

// the cut at MAX_RESULTS comes after every filter, so no filter discards a match that would have
// made the cut
pub(super) fn apply_filters_and_truncate(matches: &mut Vec<query_match>, opts: &query_opts) {
  if let Some(threshold) = opts.min_quality {
    matches.retain(|m| match_quality(m) >= threshold);
  }
  if let Some(b) = opts.bounding.as_ref() {
    // the rtree only tested the envelope, which does not imply the match's final point is inside
    // the geometry; this exact containment is the authoritative one
    matches.retain(|m| b.contains(m.latitude, m.longitude));
  }
  if let Some(levels) = opts.last_admin_levels.as_deref() {
    matches.retain(|m| leaf_level_of(m).is_some_and(|leaf| levels.iter().any(|l| l.value() == leaf)));
  }
  matches.truncate(MAX_RESULTS as usize);
}

fn leaf_level_of(m: &query_match) -> Option<u8> {
  let presumed = m
    .house_number
    .as_ref()
    .is_some_and(|number| number.kind != house_number_scenario::from_osm_data);
  m.admin_levels
    .iter()
    .rev()
    .find(|a| !(presumed && a.level == level::house_number.value()))
    .map(|a| a.level)
}

pub fn parse_min_quality(text: &str) -> Result<f64, String> {
  let value: f64 = text
    .trim()
    .parse()
    .map_err(|e| format!("quality: not a number: {e}"))?;
  if !(0.0..=1.0).contains(&value) {
    return Err(format!("quality: must be between 0.0 and 1.0, got {value}"));
  }
  Ok(value)
}

pub(super) fn coordinate_quality(distance_in_meters: Option<u32>) -> f64 {
  match distance_in_meters {
    Some(d) => (1.0 - d as f64 / COORDINATE_QUALITY_REFERENCE_M).clamp(0.0, 1.0),
    None => 1.0,
  }
}

fn match_quality(m: &query_match) -> f64 {
  if let Some(s) = m.similarity {
    return s as f64;
  }
  coordinate_quality(m.coordinates_distance_in_meters)
}
