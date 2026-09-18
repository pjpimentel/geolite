use geo::Point;
use rusqlite::Connection;

use super::entity::{
  self, leaf, match_sources, query_match, query_match_attributes, query_output, query_service,
  round5,
};
use super::filter;
use super::{house_number, query_opts};
use crate::domain::admin_level::geometry::bounding_box;
use crate::domain::admin_level::spatial_index;

const WORLD_BOUNDING_BOX: bounding_box = bounding_box {
  min_lat: -90.0,
  max_lat: 90.0,
  min_lon: -180.0,
  max_lon: 180.0,
};

pub(super) fn run(conn: &Connection, latitude: f64, longitude: f64, opts: &query_opts) -> query_output {
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
  let sources = match_sources::load(conn, &candidate_ids, opts.include_wkt);

  let mut matches: Vec<query_match> = Vec::new();
  for c in &candidates {
    let own_meta = sources.meta.get(&c.id);
    let own_name = own_meta.map(|m| m.name.as_str()).unwrap_or_default();
    // a street inside two neighbourhoods is two answers, in the order the paths are enumerated
    for path in sources.paths_of(c.id) {
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
      let admin_levels = sources.level_ladder(&ancestors, &leaf);
      let friendly_name = entity::friendly_name_of(
        opts.friendly_name_format,
        &admin_levels,
        &sources,
        c.id,
        own_name,
        path,
      );

      matches.push(query_match {
        admin_levels,
        latitude: round5(c.closest_point.y()),
        longitude: round5(c.closest_point.x()),
        coordinates_distance_in_meters: c.distance_in_meters,
        similarity: None,
        score: None,
        friendly_name,
        attributes: query_match_attributes {
          country_iso_3166_1_alpha_2_code: entity::country_iso_of(&ancestors, own_meta),
          post_code: entity::post_code_of(&ancestors)
            .or_else(|| own_meta.and_then(|m| m.post_code.clone())),
        },
        house_number: None,
        id: entity::path_id(c.id, path),
        admin_level_id: Some(c.id),
      });
    }
  }

  house_number::enrich_house_numbers(conn, input_pt, &mut matches, opts.friendly_name_format);

  filter::apply_filters_and_truncate(&mut matches, opts);

  query_output {
    service: query_service::coordinates_to_address,
    matches,
  }
}
