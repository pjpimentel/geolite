use geo::{Closest, ClosestPoint, Geometry, HaversineDistance, LineString, Point};
use rusqlite::Connection;

use super::entity::{
  self, leaf, match_sources, query_match, query_match_attributes, query_output, query_service,
  round5,
};
use super::filter::{self, bounding_geometry};
use super::{house_number, query_opts};
use crate::domain::admin_level::geometry::bounding_box;
use crate::domain::admin_level::level;

const RTREE_DELTA_DEG: f64 = 0.1;

const WORLD_BOUNDING_BOX: bounding_box = bounding_box {
  min_lat: -90.0,
  max_lat: 90.0,
  min_lon: -180.0,
  max_lon: 180.0,
};

struct admin_candidate {
  id: i64,
  admin_level: level,
  closest_point: Point<f64>,
  distance_in_meters: Option<u32>,
}

fn best_admin_levels(
  conn: &Connection,
  input_pt: Point<f64>,
  bounding: Option<&bounding_geometry>,
) -> Vec<admin_candidate> {
  let (lon, lat) = (input_pt.x(), input_pt.y());
  let envelope = bounding.map(|b| b.envelope).unwrap_or(WORLD_BOUNDING_BOX);
  let raw = crate::domain::admin_level::repository::streets_for_coordinates(
    conn,
    lon,
    lat,
    RTREE_DELTA_DEG,
    envelope,
  );
  crate::debug!("debug: rtree raw={} for lon={} lat={}", raw.len(), lon, lat);

  let mut rej_wkt_none = 0;
  let mut rej_not_linestring = 0;
  let mut rej_empty = 0;
  let mut rej_indeterminate = 0;
  let mut rej_outside_polygon = 0;
  let mut min_dist = f64::MAX;

  let mut candidates: Vec<admin_candidate> = Vec::new();
  for s in raw {
    let geom = match s.wkb {
      Some(g) => g.into_geometry(),
      None => {
        rej_wkt_none += 1;
        continue;
      }
    };
    let linestrings: Vec<LineString<f64>> = match geom {
      Geometry::LineString(ls) => vec![ls],
      Geometry::MultiLineString(mls) => mls.0,
      _ => {
        rej_not_linestring += 1;
        continue;
      }
    };
    let mut best: Option<(Point<f64>, f64)> = None;
    let mut had_non_empty = false;
    for ls in &linestrings {
      if ls.0.is_empty() {
        continue;
      }
      had_non_empty = true;
      if let Closest::SinglePoint(p) | Closest::Intersection(p) = ls.closest_point(&input_pt) {
        let d = input_pt.haversine_distance(&p);
        if best.is_none_or(|(_, bd)| d < bd) {
          best = Some((p, d));
        }
      }
    }
    if !had_non_empty {
      rej_empty += 1;
      continue;
    }
    let (cp, dist) = match best {
      Some(b) => b,
      None => {
        rej_indeterminate += 1;
        continue;
      }
    };
    // the polygon test runs on the candidates, before the heavy loads: the coordinate service
    // never moves a match's point, so filtering here equals filtering at the end
    if let Some(b) = bounding
      && !b.contains(cp.y(), cp.x())
    {
      rej_outside_polygon += 1;
      continue;
    }
    if dist < min_dist {
      min_dist = dist;
    }
    let distance_in_meters = Some(dist.round().clamp(0.0, u32::MAX as f64) as u32);
    candidates.push(admin_candidate {
      id: s.id,
      admin_level: s.admin_level,
      closest_point: cp,
      distance_in_meters,
    });
  }

  crate::debug!(
    "debug: rejections wkt_none={} not_linestring={} empty={} indeterminate={} outside_polygon={} min_dist_m={:.2}",
    rej_wkt_none,
    rej_not_linestring,
    rej_empty,
    rej_indeterminate,
    rej_outside_polygon,
    min_dist
  );

  candidates.sort_by(|a, b| {
    b.admin_level
      .cmp(&a.admin_level)
      .then(a.distance_in_meters.cmp(&b.distance_in_meters))
  });
  candidates
}

pub(super) fn run(conn: &Connection, latitude: f64, longitude: f64, opts: &query_opts) -> query_output {
  let input_pt = Point::new(longitude, latitude);

  let mut candidates = best_admin_levels(conn, input_pt, opts.bounding.as_ref());
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
    let hierarchy = sources.hierarchies.get(&c.id);
    // the chain comes most-specific first; reversed before the stable sort so that, within one
    // level, the order is general → specific
    let ancestors = sources.ancestors_of(sources.chain_of(c.id).iter().rev());
    let leaf = leaf {
      id: c.id,
      level: c.admin_level,
      name: own_name,
      relation_id: own_meta.and_then(|m| m.relation_id),
      way_id: own_meta.and_then(|m| m.way_id),
    };
    let admin_levels = sources.level_ladder(&ancestors, &leaf);
    let friendly_name =
      entity::friendly_name_of(opts.friendly_name_format, &admin_levels, hierarchy, own_name);

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
      id: c.id as u64,
      admin_level_id: Some(c.id),
    });
  }

  house_number::enrich_house_numbers(conn, input_pt, &mut matches, opts.friendly_name_format);

  filter::apply_filters_and_truncate(&mut matches, opts);

  query_output {
    service: query_service::coordinates_to_address,
    matches,
  }
}

#[cfg(test)]
#[path = "coordinates.test.rs"]
mod tests;
