use geo::{Intersects, Point};
use rusqlite::Connection;

use super::entity::{leaf, match_sources, query_match, query_output, query_service};
use super::filter;
use super::{house_number, query_opts};
use crate::admin_level::geometry::bounding_box;
use crate::admin_level::repository as admin_level_repository;
use crate::admin_level::spatial_index;
use crate::house_number::house_number_policy;

const WORLD_BOUNDING_BOX: bounding_box = bounding_box {
  min_lat: -90.0,
  max_lat: 90.0,
  min_lon: -180.0,
  max_lon: 180.0,
};

fn paths_at<'a>(
  conn: &Connection,
  sources: &'a match_sources,
  id: i64,
  point: &Point<f64>,
) -> Vec<&'a Vec<i64>> {
  let paths = sources.paths_of(id);
  if paths.len() < 2 {
    return paths.iter().collect();
  }
  let boxed: Vec<&Vec<i64>> = paths
    .iter()
    .filter(|path| path.first().is_some_and(|&leaf| sources.box_covers(leaf, point)))
    .collect();
  let leaves: Vec<i64> = boxed.iter().filter_map(|path| path.first().copied()).collect();
  let areas = admin_level_repository::geometry_by_ids(conn, &leaves);
  let inside: Vec<&Vec<i64>> = boxed
    .into_iter()
    .filter(|path| {
      areas
        .iter()
        .any(|(leaf, area)| path.first() == Some(leaf) && area.geometry().intersects(point))
    })
    .collect();
  if inside.is_empty() {
    paths.iter().collect()
  } else {
    inside
  }
}

pub(super) fn run(
  conn: &Connection,
  house_numbers: &house_number_policy,
  latitude: f64,
  longitude: f64,
  opts: &query_opts,
) -> query_output {
  let input_pt = Point::new(longitude, latitude);

  let envelope = opts.bounding.as_ref().map(|b| b.envelope).unwrap_or(WORLD_BOUNDING_BOX);
  let mut candidates = spatial_index::nearest(conn, input_pt, envelope);
  // the polygon test runs on the candidates, before the heavy loads: the coordinate service
  // never moves a match's point, so filtering here equals filtering at the end
  if let Some(b) = opts.bounding.as_ref() {
    candidates.retain(|c| b.contains(c.closest_point.y(), c.closest_point.x()));
  }
  if let Some(threshold) = opts.min_quality {
    candidates.retain(|c| filter::coordinate_quality(c.distance_in_meters) >= threshold);
  }
  // the cut before the loads is the cheap path; last_admin_levels reads the leaf after the
  // house-number step (level 30), so it keeps every candidate until the end
  if opts.last_admin_levels.is_none() {
    candidates.truncate(filter::MAX_RESULTS as usize);
  }
  crate::debug!("debug: candidates={}", candidates.len());
  let candidate_ids: Vec<i64> = candidates.iter().map(|c| c.id).collect();
  let mut sources = match_sources::load(conn, &candidate_ids, opts.include_wkt);
  sources.load_leaf_boxes(conn);
  let numbers = house_number::at_point(conn, input_pt, &candidates, house_numbers);

  let mut matches: Vec<query_match> = Vec::new();
  for c in &candidates {
    let own_meta = sources.meta.get(&c.id);
    let own_name = own_meta.map(|m| m.name.as_str()).unwrap_or_default();
    let reported = numbers.get(&c.id).map(house_number::reported);
    // a street inside two neighbourhoods is two answers, in the order the paths are enumerated
    for path in paths_at(conn, &sources, c.id, &c.closest_point) {
      // the path comes most-specific first; reversed before the stable sort so that, within one
      // level, the order is general → specific
      let ancestors = sources.ancestors_of(path.iter().rev());
      let leaf = leaf {
        id: c.id,
        level: c.level,
        name: own_name,
        relation_id: own_meta.and_then(|m| m.relation_id),
        way_id: own_meta.and_then(|m| m.way_id),
      };
      matches.push(query_match {
        coordinates_distance_in_meters: c.distance_in_meters,
        ..sources.match_at(
          &leaf,
          &ancestors,
          path,
          c.closest_point,
          reported.as_ref(),
          opts.friendly_name_format,
        )
      });
    }
  }

  filter::apply_filters_and_truncate(&mut matches, opts);

  query_output {
    service: query_service::coordinates_to_address,
    matches,
  }
}
