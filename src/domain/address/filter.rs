use geo::{Contains, Point};

use super::entity::query_match;
use super::query_opts;
use crate::domain::admin_level::geometry::bounding_box;

pub(super) const MAX_RESULTS: u8 = 10;
const COORDINATE_QUALITY_REFERENCE_M: f64 = 100.0;

#[derive(Clone)]
pub struct bounding_geometry {
  pub geometry: geo::Geometry<f64>,
  pub envelope: bounding_box,
}

impl bounding_geometry {
  pub fn contains(&self, lat: f64, lon: f64) -> bool {
    self.geometry.contains(&Point::new(lon, lat))
  }

  #[cfg(test)]
  pub(crate) fn from_rect(b: bounding_box) -> Self {
    let ring = geo::LineString(vec![
      geo::Coord { x: b.min_lon, y: b.min_lat },
      geo::Coord { x: b.max_lon, y: b.min_lat },
      geo::Coord { x: b.max_lon, y: b.max_lat },
      geo::Coord { x: b.min_lon, y: b.max_lat },
      geo::Coord { x: b.min_lon, y: b.min_lat },
    ]);
    Self {
      geometry: geo::Geometry::Polygon(geo::Polygon::new(ring, vec![])),
      envelope: b,
    }
  }
}

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
    matches.retain(|m| {
      m.admin_levels
        .last()
        .is_some_and(|a| levels.iter().any(|l| l.value() == a.level))
    });
  }
  matches.truncate(MAX_RESULTS as usize);
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

#[cfg(test)]
#[path = "filter.test.rs"]
mod tests;
