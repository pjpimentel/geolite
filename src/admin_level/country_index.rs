use std::sync::OnceLock;

use geo::{Centroid, CoordsIter, EuclideanDistance, Geometry, Point, Polygon};
use rstar::{AABB, RTree};

use super::geometry::parse_bounding_wkt;
use super::point_in_polygon::{
  classify_in_polygons, extract_polygons, point_class, polygon_entry, spatial_entry,
};

struct country_entry {
  iso: &'static str,
  polys: Vec<polygon_entry>,
  geometry: Geometry<f64>,
}

impl country_entry {
  fn outline_distance(&self, point: Point<f64>) -> f64 {
    let polygons: Vec<&Polygon<f64>> = match &self.geometry {
      Geometry::Polygon(polygon) => vec![polygon],
      Geometry::MultiPolygon(polygons) => polygons.0.iter().collect(),
      _ => vec![],
    };
    polygons
      .iter()
      .map(|polygon| point.euclidean_distance(polygon.exterior()))
      .fold(f64::MAX, f64::min)
  }
}

pub struct country_index {
  entries: Vec<country_entry>,
  tree: RTree<spatial_entry>,
}

static SHARED: OnceLock<country_index> = OnceLock::new();

impl country_index {
  pub fn shared() -> &'static country_index {
    SHARED.get_or_init(country_index::from_boundaries)
  }

  pub fn from_boundaries() -> Self {
    let mut entries: Vec<country_entry> = Vec::new();
    let mut boxes: Vec<spatial_entry> = Vec::new();
    for country in crate::presets::boundaries::ALL {
      let boundary = parse_bounding_wkt(country.boundary_wkt)
        .unwrap_or_else(|e| panic!("the boundary of {} does not parse: {e}", country.iso_3166_1_alpha_2));
      let envelope = boundary.envelope;
      boxes.push(spatial_entry {
        idx: entries.len(),
        envelope: AABB::from_corners(
          [envelope.min_lon, envelope.min_lat],
          [envelope.max_lon, envelope.max_lat],
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
  // shape or one drawn along a border, then the nearest country: a pier, a harbour, a street over
  // the water or an island no outline holds belongs to the country closest to it
  pub fn country_of(&self, geometry: &Geometry<f64>) -> Option<&'static str> {
    let centroid = geometry.centroid().map(|p| [p.x(), p.y()]);
    let first_vertex = geometry.coords_iter().next().map(|c| [c.x, c.y]);
    centroid
      .and_then(|point| self.inside(point))
      .or_else(|| first_vertex.and_then(|point| self.inside(point)))
      .or_else(|| centroid.or(first_vertex).and_then(|point| self.nearest(point)))
  }

  // the rule of a row, so a point is looked for in the base its street was written to
  pub fn country_at(&self, longitude: f64, latitude: f64) -> Option<&'static str> {
    self
      .inside([longitude, latitude])
      .or_else(|| self.nearest([longitude, latitude]))
  }

  // the outlines of two neighbours overlap along their border, each drawn a few kilometres into
  // the other, so a point inside both belongs to the country whose outline is farthest from it
  fn inside(&self, [x, y]: [f64; 2]) -> Option<&'static str> {
    let mut on_edge: Option<&'static str> = None;
    let mut holding: Vec<&country_entry> = Vec::new();
    for entry in self.tree.locate_in_envelope_intersecting(&AABB::from_point([x, y])) {
      let country = &self.entries[entry.idx];
      match classify_in_polygons(x, y, &country.polys) {
        point_class::inside => holding.push(country),
        point_class::on_edge => {
          if on_edge.is_none() {
            on_edge = Some(country.iso);
          }
        }
        point_class::outside => {}
      }
    }
    match holding.as_slice() {
      [] => on_edge,
      [country] => Some(country.iso),
      _ => {
        let point = Point::new(x, y);
        holding
          .iter()
          .map(|country| (country.iso, country.outline_distance(point)))
          .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
          .map(|(iso, _)| iso)
      }
    }
  }

  fn nearest(&self, [x, y]: [f64; 2]) -> Option<&'static str> {
    let point = Point::new(x, y);
    self
      .entries
      .iter()
      .map(|country| (country.iso, point.euclidean_distance(&country.geometry)))
      .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
      .map(|(iso, _)| iso)
  }
}
