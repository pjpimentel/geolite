use geo::{Coord, Geometry, LineString, MultiPolygon, Polygon, Winding};
use rusqlite::Connection;

use super::entity::admin_level;
use super::extract::{CHUNK_SIZE, progress_report};
use super::geometry::approx_eq;
use super::rules::{extraction_rules, resolve_rules};
use super::scale::level;

struct way_meta {
  name: String,
  post_code: Option<String>,
}

struct way_work {
  way_id: u64,
  meta: way_meta,
  coords: Vec<Coord<f64>>,
  level: level,
}

pub(super) fn run(
  conn: &Connection,
  rules: &[extraction_rules],
  name_priority: &[&str],
  mut progress: impl FnMut(progress_report),
) {
  let neighborhood = level::neighborhood.value();
  let (include, _) = resolve_rules(level::neighborhood, rules);
  let mut candidate_ids: Vec<u64> = Vec::new();
  for filter in include {
    let ids =
      crate::database::osm_ways::remaining_ids_by_tags(conn, neighborhood, std::slice::from_ref(filter));
    candidate_ids.extend(ids);
  }
  candidate_ids.sort_unstable();
  candidate_ids.dedup();

  let total = candidate_ids.len() as u64;
  progress(progress_report {
    total: Some(total),
    processed: 0,
  });
  if candidate_ids.is_empty() {
    return;
  }

  // the per-item geometry work (ring winding) is far
  // cheaper than the single-connection db read/write that bounds this stage,
  // so processing sequentially is as fast as a worker pool without the channel
  // overhead
  let mut processed: u64 = 0;
  for chunk in candidate_ids.chunks(CHUNK_SIZE) {
    let works = load_chunk(conn, chunk, level::neighborhood, name_priority);
    let mut batch: Vec<admin_level> = Vec::new();
    for w in works {
      if let Some(row) = process_one_way(w) {
        batch.push(row);
      }
    }
    processed += super::repository::batch_upsert(conn, &batch) as u64;
    progress(progress_report {
      total: Some(total),
      processed,
    });
  }
}

fn load_chunk(
  conn: &Connection,
  chunk: &[u64],
  level: level,
  name_priority: &[&str],
) -> Vec<way_work> {
  let raw_rows = crate::database::osm_ways::way_coords_chunk(conn, chunk, name_priority);

  let mut way_order: Vec<u64> = Vec::new();
  let mut way_metas: std::collections::HashMap<u64, way_meta> = std::collections::HashMap::new();
  let mut way_coords: std::collections::HashMap<u64, Vec<Coord<f64>>> =
    std::collections::HashMap::new();

  for row in &raw_rows {
    if let std::collections::hash_map::Entry::Vacant(e) = way_metas.entry(row.way_id) {
      way_order.push(row.way_id);
      e.insert(way_meta {
        name: row.way_name.clone(),
        post_code: row.post_code.clone(),
      });
      way_coords.insert(row.way_id, Vec::new());
    }
    way_coords.get_mut(&row.way_id).unwrap().push(Coord {
      x: row.lon,
      y: row.lat,
    });
  }

  let mut works = Vec::with_capacity(way_order.len());
  for way_id in way_order {
    let meta = way_metas.remove(&way_id).unwrap();
    let coords = way_coords.remove(&way_id).unwrap();
    works.push(way_work {
      way_id,
      meta,
      coords,
      level,
    });
  }
  works
}

fn process_one_way(w: way_work) -> Option<admin_level> {
  if w.coords.is_empty() {
    return None;
  }

  let ls = LineString(w.coords);
  let geom: Geometry<f64> =
    if ls.0.len() >= 4 && approx_eq(ls.0[0], *ls.0.last().unwrap()) {
      let mut ring = ls;
      // spatialite st_buildarea reverses ccw rings to cw — replicate that behavior
      ring.make_cw_winding();
      Geometry::MultiPolygon(MultiPolygon(vec![Polygon::new(ring, vec![])]))
    } else {
      Geometry::LineString(ls)
    };

  Some(admin_level {
    relation_id: None,
    way_id: Some(w.way_id),
    level: w.level,
    name: w.meta.name,
    country_iso_code: None,
    post_code: w.meta.post_code,
    wkb: geom.into(),
  })
}

#[cfg(test)]
#[path = "place_ways.test.rs"]
mod tests;
