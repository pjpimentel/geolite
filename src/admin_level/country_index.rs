use geo::{Centroid, CoordsIter, EuclideanDistance, Geometry, Point};
use rstar::{AABB, RTree};

use super::geometry::parse_bounding_wkt;
use super::point_in_polygon::{
  classify_in_polygons, extract_polygons, point_class, polygon_entry, spatial_entry,
};

// a coastline drawn at 1:10m leaves a pier, a harbour or a street over the water outside the
// polygon: a point this close to a country is the country's. small on purpose, so that a town of a
// country without a preset only falls to its neighbour within the same reach of the border
const COAST_REACH_DEG: f64 = 0.05;

struct country_entry {
  iso: &'static str,
  polys: Vec<polygon_entry>,
  geometry: Geometry<f64>,
}

pub struct country_index {
  entries: Vec<country_entry>,
  tree: RTree<spatial_entry>,
}

impl country_index {
  pub fn from_presets() -> Self {
    let mut entries: Vec<country_entry> = Vec::new();
    let mut boxes: Vec<spatial_entry> = Vec::new();
    for preset in crate::presets::COUNTRIES {
      let Some(country) = preset.country else {
        continue;
      };
      let boundary = parse_bounding_wkt(country.boundary_wkt)
        .unwrap_or_else(|e| panic!("the boundary of {} does not parse: {e}", country.iso_3166_1_alpha_2));
      let envelope = boundary.envelope;
      boxes.push(spatial_entry {
        idx: entries.len(),
        envelope: AABB::from_corners(
          [envelope.min_lon - COAST_REACH_DEG, envelope.min_lat - COAST_REACH_DEG],
          [envelope.max_lon + COAST_REACH_DEG, envelope.max_lat + COAST_REACH_DEG],
        ),
      });
      entries.push(country_entry {
        iso: country.iso_3166_1_alpha_2,
        polys: extract_polygons(boundary.geometry.clone()),
        geometry: boundary.geometry,
      });
    }
    country_index {
      entries,
      tree: RTree::bulk_load(boxes),
    }
  }

  // the centroid decides, then the first vertex when the centroid falls in no country, a concave
  // shape or one drawn along a border, then the nearest country within reach of the coast
  pub fn country_of(&self, geometry: &Geometry<f64>) -> Option<&'static str> {
    let centroid = geometry.centroid().map(|p| [p.x(), p.y()]);
    let first_vertex = geometry.coords_iter().next().map(|c| [c.x, c.y]);
    centroid
      .and_then(|point| self.inside(point))
      .or_else(|| first_vertex.and_then(|point| self.inside(point)))
      .or_else(|| centroid.and_then(|point| self.within_reach(point)))
  }

  fn inside(&self, [x, y]: [f64; 2]) -> Option<&'static str> {
    let mut on_edge: Option<&'static str> = None;
    for entry in self.tree.locate_in_envelope_intersecting(&AABB::from_point([x, y])) {
      let country = &self.entries[entry.idx];
      match classify_in_polygons(x, y, &country.polys) {
        point_class::inside => return Some(country.iso),
        point_class::on_edge => {
          if on_edge.is_none() {
            on_edge = Some(country.iso);
          }
        }
        point_class::outside => {}
      }
    }
    on_edge
  }

  fn within_reach(&self, [x, y]: [f64; 2]) -> Option<&'static str> {
    let point = Point::new(x, y);
    self
      .tree
      .locate_in_envelope_intersecting(&AABB::from_point([x, y]))
      .map(|entry| &self.entries[entry.idx])
      .map(|country| (country.iso, point.euclidean_distance(&country.geometry)))
      .filter(|(_, distance)| *distance <= COAST_REACH_DEG)
      .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
      .map(|(iso, _)| iso)
  }
}
