use geo::{Coord, Geometry, LineString, MultiPolygon, Polygon};

use crate::admin_level::id::admin_level_id;
use crate::admin_level::{admin_level, level};

pub(crate) fn ring(min: f64, max: f64) -> LineString<f64> {
  LineString(vec![
    Coord { x: min, y: min },
    Coord { x: max, y: min },
    Coord { x: max, y: max },
    Coord { x: min, y: max },
    Coord { x: min, y: min },
  ])
}

pub(crate) fn area(relation_id: u64, level: level, name: &str, min: f64, max: f64) -> admin_level {
  admin_level {
    relation_id: Some(relation_id),
    way_id: None,
    level,
    wkb: Geometry::MultiPolygon(MultiPolygon(vec![Polygon::new(ring(min, max), vec![])])).into(),
    name: name.to_string(),
    country_iso_code: None,
    post_code: None,
  }
}

pub(crate) fn street(way_id: u64, name: &str, at: f64) -> admin_level {
  admin_level {
    relation_id: None,
    way_id: Some(way_id),
    level: level::street,
    wkb: Geometry::LineString(LineString(vec![
      Coord { x: at, y: at },
      Coord { x: at + 0.1, y: at },
    ]))
    .into(),
    name: name.to_string(),
    country_iso_code: None,
    post_code: None,
  }
}

pub(crate) fn relation(osm_id: u64) -> i64 {
  admin_level_id::from_relation(osm_id).raw() as i64
}

pub(crate) fn way(osm_id: u64) -> i64 {
  admin_level_id::from_way(osm_id).raw() as i64
}

fn rect(min: [f64; 2], max: [f64; 2]) -> LineString<f64> {
  LineString(vec![
    Coord { x: min[0], y: min[1] },
    Coord { x: max[0], y: min[1] },
    Coord { x: max[0], y: max[1] },
    Coord { x: min[0], y: max[1] },
    Coord { x: min[0], y: min[1] },
  ])
}

pub(crate) fn area_at(
  relation_id: u64,
  level: level,
  name: &str,
  min: [f64; 2],
  max: [f64; 2],
) -> admin_level {
  admin_level {
    relation_id: Some(relation_id),
    way_id: None,
    level,
    wkb: Geometry::MultiPolygon(MultiPolygon(vec![Polygon::new(rect(min, max), vec![])])).into(),
    name: name.to_string(),
    country_iso_code: None,
    post_code: None,
  }
}

pub(crate) fn street_between(way_id: u64, name: &str, from: [f64; 2], to: [f64; 2]) -> admin_level {
  admin_level {
    relation_id: None,
    way_id: Some(way_id),
    level: level::street,
    wkb: Geometry::LineString(LineString(vec![
      Coord { x: from[0], y: from[1] },
      Coord { x: to[0], y: to[1] },
    ]))
    .into(),
    name: name.to_string(),
    country_iso_code: None,
    post_code: None,
  }
}
