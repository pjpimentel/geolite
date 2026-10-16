use geo::{Geometry, LineString};
use rstar::{AABB, RTreeObject};

pub(crate) struct spatial_entry {
  pub(crate) idx: usize,
  pub(crate) envelope: AABB<[f64; 2]>,
}

impl RTreeObject for spatial_entry {
  type Envelope = AABB<[f64; 2]>;
  fn envelope(&self) -> Self::Envelope {
    self.envelope
  }
}

// a ring above this many edges is asked about often enough to pay for an index, and the bands
// are sized so that a point test walks a few dozen edges instead of the whole boundary of a state
const RING_INDEX_MIN_EDGES: usize = 512;
const RING_BAND_EDGES: usize = 32;
const MAX_BANDS: usize = 4_096;

struct ring {
  points: Vec<[f64; 2]>,
  min_y: f64,
  height: f64,
  bands: Vec<Vec<u32>>,
}

impl ring {
  fn new(points: Vec<[f64; 2]>) -> Self {
    let (min_y, max_y) = points.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
      (lo.min(p[1]), hi.max(p[1]))
    });
    let height = if points.is_empty() { 0.0 } else { max_y - min_y };
    let edges = points.len();
    let mut bands: Vec<Vec<u32>> = Vec::new();
    if edges >= RING_INDEX_MIN_EDGES && height > 0.0 {
      let count = (edges / RING_BAND_EDGES).clamp(1, MAX_BANDS);
      bands = vec![Vec::new(); count];
      for edge in 0..edges {
        let (a, b) = (points[edge][1], points[(edge + 1) % edges][1]);
        let first = band_of(a.min(b), min_y, height, count);
        let last = band_of(a.max(b), min_y, height, count);
        for band in &mut bands[first..=last] {
          band.push(edge as u32);
        }
      }
    }
    ring {
      points,
      min_y,
      height,
      bands,
    }
  }
}

fn band_of(y: f64, min_y: f64, height: f64, count: usize) -> usize {
  if height <= 0.0 {
    return 0;
  }
  let raw = (y - min_y) / height * count as f64;
  (raw.max(0.0) as usize).min(count - 1)
}

pub(crate) struct polygon_entry {
  exterior: ring,
  interiors: Vec<ring>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum point_class {
  inside,
  on_edge,
  outside,
}

pub(crate) fn extract_polygons(geometry: Geometry<f64>) -> Vec<polygon_entry> {
  match geometry {
    Geometry::Polygon(p) => vec![polygon_to_entry(p)],
    Geometry::MultiPolygon(mp) => mp.0.into_iter().map(polygon_to_entry).collect(),
    _ => vec![],
  }
}

fn polygon_to_entry(p: geo::Polygon<f64>) -> polygon_entry {
  let (exterior, interiors) = p.into_inner();
  let points_of = |ls: LineString<f64>| ls.0.into_iter().map(|c| [c.x, c.y]).collect();
  polygon_entry {
    exterior: ring::new(points_of(exterior)),
    interiors: interiors.into_iter().map(points_of).map(ring::new).collect(),
  }
}

pub(crate) fn classify_in_polygons(px: f64, py: f64, polys: &[polygon_entry]) -> point_class {
  let mut touched_edge = false;
  for poly in polys {
    match classify_point(px, py, &poly.exterior) {
      point_class::outside => continue,
      point_class::on_edge => touched_edge = true,
      point_class::inside => {
        let mut in_hole = false;
        for hole in &poly.interiors {
          match classify_point(px, py, hole) {
            point_class::inside => in_hole = true,
            point_class::on_edge => {
              touched_edge = true;
              in_hole = true;
            }
            point_class::outside => continue,
          }
          break;
        }
        if !in_hole {
          return point_class::inside;
        }
      }
    }
  }
  if touched_edge {
    point_class::on_edge
  } else {
    point_class::outside
  }
}

fn classify_point(px: f64, py: f64, ring: &ring) -> point_class {
  let n = ring.points.len();
  if n < 3 {
    return point_class::outside;
  }
  let mut inside = false;
  if ring.bands.is_empty() {
    for edge in 0..n {
      if crosses(px, py, ring.points[edge], ring.points[(edge + 1) % n], &mut inside) {
        return point_class::on_edge;
      }
    }
  } else {
    // only the edges of the band the point falls in can cross a ray at its latitude
    let band = band_of(py, ring.min_y, ring.height, ring.bands.len());
    for &edge in &ring.bands[band] {
      let edge = edge as usize;
      if crosses(px, py, ring.points[edge], ring.points[(edge + 1) % n], &mut inside) {
        return point_class::on_edge;
      }
    }
  }
  if inside {
    point_class::inside
  } else {
    point_class::outside
  }
}

// answers whether the point lies on the edge; otherwise flips `inside` when a ray to the left of
// the point crosses it
fn crosses(px: f64, py: f64, a: [f64; 2], b: [f64; 2], inside: &mut bool) -> bool {
  if on_segment(px, py, a, b) {
    return true;
  }
  let ([xj, yj], [xi, yi]) = (a, b);
  if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
    *inside = !*inside;
  }
  false
}

// a vertex osm shares between a border and a street lands on the segment exactly, so the
// tolerance only has to cover the rounding of the cross product
fn on_segment(px: f64, py: f64, a: [f64; 2], b: [f64; 2]) -> bool {
  const EPSILON: f64 = 1e-9;

  let ([ax, ay], [bx, by]) = (a, b);
  if px < ax.min(bx) - EPSILON
    || px > ax.max(bx) + EPSILON
    || py < ay.min(by) - EPSILON
    || py > ay.max(by) + EPSILON
  {
    return false;
  }
  let cross = (bx - ax) * (py - ay) - (by - ay) * (px - ax);
  let length = ((bx - ax).powi(2) + (by - ay).powi(2)).sqrt();
  cross.abs() <= EPSILON * length.max(EPSILON)
}
