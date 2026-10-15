use geo::{Coord, Geometry, LineString, MultiPolygon, Polygon, Winding};
use rusqlite::Connection;

use super::entity::admin_level;
use super::extract::{CHUNK_SIZE, extract_opts, identity_of, known_way_ids, write_batch, written};
use super::geometry::approx_eq;
use super::id::admin_level_id;
use super::rules::rules_of;
use super::scale::level;
use crate::database::store::base_pool;
use crate::progress_report;

struct way_meta {
  names: Vec<Option<String>>,
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
  opts: &extract_opts,
  pool: &mut base_pool,
  mut progress: impl FnMut(progress_report),
) -> written {
  let (include, _) = rules_of(level::neighborhood);
  let known = known_way_ids(pool, level::neighborhood);
  let mut candidate_ids: Vec<u64> = Vec::new();
  for filter in include {
    let ids = crate::osm_way::repository::candidate_ids_by_tags(conn, std::slice::from_ref(filter));
    candidate_ids.extend(ids.into_iter().filter(|id| !known.contains(id)));
  }
  candidate_ids.sort_unstable();
  candidate_ids.dedup();

  let total = candidate_ids.len() as u64;
  progress(progress_report {
    total: Some(total),
    processed: 0,
  });
  let mut written = written {
    kept: 0,
    dropped: 0,
  };
  if candidate_ids.is_empty() {
    return written;
  }

  // the per-item geometry work (ring winding) is far
  // cheaper than the single-connection db read/write that bounds this stage,
  // so processing sequentially is as fast as a worker pool without the channel
  // overhead
  let mut processed: u64 = 0;
  for chunk in candidate_ids.chunks(CHUNK_SIZE) {
    let works = load_chunk(conn, chunk, level::neighborhood, opts.name_tags);
    processed += works.len() as u64;
    let mut batch: Vec<admin_level> = Vec::new();
    for w in works {
      if let Some(row) = process_one_way(w, opts) {
        batch.push(row);
      }
    }
    let wrote = write_batch(pool, batch);
    written.kept += wrote.kept;
    written.dropped += wrote.dropped;
    progress(progress_report {
      total: Some(total),
      processed,
    });
  }
  written
}

fn load_chunk(
  conn: &Connection,
  chunk: &[u64],
  level: level,
  name_tags: &[&str],
) -> Vec<way_work> {
  let raw_rows = crate::osm_way::repository::way_coords_chunk(conn, chunk, name_tags);

  let mut way_order: Vec<u64> = Vec::new();
  let mut way_metas: std::collections::HashMap<u64, way_meta> = std::collections::HashMap::new();
  let mut way_coords: std::collections::HashMap<u64, Vec<Coord<f64>>> =
    std::collections::HashMap::new();

  for row in &raw_rows {
    if let std::collections::hash_map::Entry::Vacant(e) = way_metas.entry(row.way_id) {
      way_order.push(row.way_id);
      e.insert(way_meta {
        names: row.names.clone(),
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

fn process_one_way(w: way_work, opts: &extract_opts) -> Option<admin_level> {
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

  let identity = identity_of(opts, w.level, &geom, &w.meta.names)?;
  Some(admin_level {
    id: admin_level_id::from_way(w.way_id),
    level: w.level,
    name: identity.name,
    country_iso_code: Some(identity.iso.to_string()),
    post_code: w.meta.post_code,
    wkb: geom.into(),
  })
}
