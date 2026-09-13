use geo::{Geometry, LineString, Point};
use rusqlite::Connection;

use super::entity::house_number_link;
use super::policy::{COMPOUND_SHAPES, house_number_policy};
use super::strategy::link_strategy;
use super::value::house_number;
use crate::domain::admin_level::{admin_level, admin_level_id, level};

pub(crate) const BR_DROPS: &[&str] = &["s/n", "sn", "s/nº", "s/no"];

pub(crate) fn simple_policy() -> house_number_policy {
  crate::presets::DEFAULT.house_numbers
}

pub(crate) fn dropping_policy() -> house_number_policy {
  house_number_policy {
    drop_values: BR_DROPS,
    ..simple_policy()
  }
}

pub(crate) fn compound_policy() -> house_number_policy {
  house_number_policy {
    shapes: COMPOUND_SHAPES,
    allow_hash_prefix: true,
    ..simple_policy()
  }
}

pub(crate) fn line(points: &[(f64, f64)]) -> LineString<f64> {
  LineString(crate::domain::pbf_fixtures::coords(points))
}

pub(crate) fn street_along(way_id: u64, name: &str, points: &[(f64, f64)]) -> admin_level {
  admin_level {
    relation_id: None,
    way_id: Some(way_id),
    level: level::street,
    wkb: Geometry::LineString(line(points)).into(),
    name: name.to_string(),
    country_iso_code: None,
    post_code: None,
  }
}

pub(crate) fn link(
  node_id: u64,
  street_id: i64,
  number: &str,
  lon: f64,
  lat: f64,
) -> house_number_link {
  house_number_link {
    node_id,
    street_id: admin_level_id::from_raw(street_id as u64),
    number: house_number::from_stored(number),
    point: Point::new(lon, lat),
    strategy: link_strategy::by_proximity,
  }
}

pub(crate) fn stored_links(conn: &Connection) -> Vec<(i64, i64, String, i64)> {
  conn
    .prepare("SELECT node_id, admin_level_id, number, strategy FROM house_numbers ORDER BY node_id")
    .expect("failed to prepare")
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read row"))
    .collect()
}
