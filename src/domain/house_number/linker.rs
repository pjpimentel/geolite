use geo::{
  BoundingRect, Closest, ClosestPoint, EuclideanDistance, Geometry, LineString, MultiLineString,
  Point,
};
use rstar::{AABB, PointDistance, RTree, RTreeObject};
use std::collections::HashMap;
use std::sync::Arc;

use super::entity::house_number_link;
use super::repository::candidate_row;
use super::strategy::link_strategy;
use crate::domain::admin_level::admin_level_id;

// approximates the legacy 3x3 grid filter at 0.05° per cell: a candidate further than this from
// the nearest street is skipped
const MAX_MATCH_DEG: f64 = 0.15;

pub(super) struct street {
  pub(super) id: i64,
  pub(super) name: String,
  geometry: MultiLineString<f64>,
  envelope: AABB<[f64; 2]>,
}

impl street {
  pub(super) fn from_geometry(id: i64, name: String, geometry: &Geometry<f64>) -> Option<Self> {
    let geometry = geometry_to_multilinestring(geometry)?;
    // geometry_to_multilinestring answers Some only when at least one line has a coordinate, and
    // then the bounding rect always exists; skipping the street silently here would hide a data
    // bug, so the invariant is explicit
    let bbox = geometry
      .bounding_rect()
      .expect("a non-empty multilinestring always has a bounding rect");
    Some(Self {
      id,
      name,
      geometry,
      envelope: AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]),
    })
  }
}

struct indexed_street {
  data: Arc<street>,
}

impl RTreeObject for indexed_street {
  type Envelope = AABB<[f64; 2]>;
  fn envelope(&self) -> Self::Envelope {
    self.data.envelope
  }
}

impl PointDistance for indexed_street {
  fn distance_2(&self, point: &[f64; 2]) -> f64 {
    let p = Point::new(point[0], point[1]);
    let d = p.euclidean_distance(&self.data.geometry);
    d * d
  }
}

pub(super) struct tile_data {
  pub(super) streets: Vec<Arc<street>>,
  pub(super) candidates: Vec<candidate_row>,
}

fn geometry_to_multilinestring(geom: &Geometry<f64>) -> Option<MultiLineString<f64>> {
  let lines: Vec<LineString<f64>> = match geom {
    Geometry::MultiLineString(mls) => mls.0.clone(),
    Geometry::LineString(ls) => vec![ls.clone()],
    Geometry::Polygon(p) => {
      let mut v = vec![p.exterior().clone()];
      v.extend(p.interiors().iter().cloned());
      v
    }
    Geometry::MultiPolygon(mp) => {
      let mut v = Vec::new();
      for p in &mp.0 {
        v.push(p.exterior().clone());
        v.extend(p.interiors().iter().cloned());
      }
      v
    }
    _ => return None,
  };
  if lines.iter().all(|ls| ls.0.is_empty()) {
    return None;
  }
  Some(MultiLineString(lines))
}

fn closest_point_on_geometry(geom: &MultiLineString<f64>, p: &Point<f64>) -> Option<Point<f64>> {
  let mut best: Option<(Point<f64>, f64)> = None;
  for ls in &geom.0 {
    if ls.0.is_empty() {
      continue;
    }
    if let Closest::SinglePoint(cp) | Closest::Intersection(cp) = ls.closest_point(p) {
      let d = p.euclidean_distance(&cp);
      if best.is_none_or(|(_, bd)| d < bd) {
        best = Some((cp, d));
      }
    }
  }
  best.map(|(cp, _)| cp)
}

pub(super) fn link_tile(tile: tile_data) -> Vec<house_number_link> {
  let tile_data {
    streets,
    candidates,
  } = tile;

  let mut by_name: HashMap<String, Vec<Arc<street>>> = HashMap::new();
  for s in &streets {
    by_name
      .entry(s.name.to_lowercase())
      .or_default()
      .push(s.clone());
  }

  let indexed: Vec<indexed_street> = streets
    .iter()
    .map(|s| indexed_street { data: s.clone() })
    .collect();
  let tree = RTree::bulk_load(indexed);

  let max_dist_sq = MAX_MATCH_DEG * MAX_MATCH_DEG;

  let mut links: Vec<house_number_link> = Vec::new();
  for c in candidates {
    let pt = Point::new(c.lon, c.lat);
    let mut best: Option<Arc<street>> = None;
    let mut strategy = link_strategy::by_proximity;

    if let Some(addr_street) = &c.addr_street
      && let Some(matches) = by_name.get(&addr_street.to_lowercase())
    {
      let mut min_d = f64::INFINITY;
      for s in matches {
        let d = pt.euclidean_distance(&s.geometry);
        if d < min_d {
          min_d = d;
          best = Some(s.clone());
        }
      }
      if best.is_some() {
        strategy = link_strategy::by_name;
      }
    }

    if best.is_none()
      && let Some(nearest) = tree.nearest_neighbor(&[c.lon, c.lat])
      && nearest.distance_2(&[c.lon, c.lat]) <= max_dist_sq
    {
      best = Some(nearest.data.clone());
    }

    if let Some(s) = best
      && let Some(cp) = closest_point_on_geometry(&s.geometry, &pt)
    {
      links.push(house_number_link {
        node_id: c.id,
        street_id: admin_level_id::from_raw(s.id as u64),
        number: c.number,
        point: cp,
        strategy,
      });
    }
  }
  links
}

#[cfg(test)]
#[path = "linker.test.rs"]
mod tests;
