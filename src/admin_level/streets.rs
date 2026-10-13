use geo::{Coord, Geometry, LineString};
use rusqlite::Connection;

use super::entity::admin_level;
use super::extract::{CHUNK_SIZE, extract_opts, identity_of};
use super::id::admin_level_id;
use super::rules::rules_of;
use super::scale::level;
use crate::progress_report;

struct way_work {
  way_id: u64,
  names: Vec<Option<String>>,
  post_code: Option<String>,
  coords: Vec<Coord<f64>>,
}

pub(super) fn run(
  conn: &Connection,
  opts: &extract_opts,
  mut progress: impl FnMut(progress_report),
) -> u64 {
  let (_, exclude) = rules_of(level::street);
  let candidate_ids =
    crate::osm_way::repository::remaining_ids_by_tags(conn, level::street, exclude);

  let total = candidate_ids.len() as u64;
  progress(progress_report {
    total: Some(total),
    processed: 0,
  });
  if candidate_ids.is_empty() {
    return 0;
  }

  // the per-item geometry work (line assembly) is far cheaper than the
  // single-connection db read/write that bounds this stage, so processing
  // sequentially is as fast as a worker pool without the channel overhead
  let mut processed: u64 = 0;
  let mut kept: u64 = 0;
  for chunk in candidate_ids.chunks(CHUNK_SIZE) {
    let works = load_chunk(conn, chunk, opts.name_tags);
    processed += works.len() as u64;
    let mut batch: Vec<admin_level> = Vec::new();
    for w in works {
      if let Some(row) = process_one_way(w, opts) {
        batch.push(row);
      }
    }
    kept += super::repository::batch_upsert(conn, &batch) as u64;
    progress(progress_report {
      total: Some(total),
      processed,
    });
  }
  kept
}

fn load_chunk(conn: &Connection, chunk: &[u64], name_tags: &[&str]) -> Vec<way_work> {
  let raw_rows = crate::osm_way::repository::way_coords_chunk(conn, chunk, name_tags);

  type way_name_tuple = (Vec<Option<String>>, Option<String>);

  let mut way_order: Vec<u64> = Vec::new();
  let mut way_names: std::collections::HashMap<u64, way_name_tuple> =
    std::collections::HashMap::new();
  let mut way_coords: std::collections::HashMap<u64, Vec<Coord<f64>>> =
    std::collections::HashMap::new();

  for row in &raw_rows {
    if let std::collections::hash_map::Entry::Vacant(e) = way_names.entry(row.way_id) {
      way_order.push(row.way_id);
      e.insert((row.names.clone(), row.post_code.clone()));
      way_coords.insert(row.way_id, Vec::new());
    }
    way_coords.get_mut(&row.way_id).unwrap().push(Coord {
      x: row.lon,
      y: row.lat,
    });
  }

  let mut works = Vec::with_capacity(way_order.len());
  for way_id in way_order {
    let (names, post_code) = way_names.remove(&way_id).unwrap();
    let coords = way_coords.remove(&way_id).unwrap();
    works.push(way_work {
      way_id,
      names,
      post_code,
      coords,
    });
  }
  works
}

fn process_one_way(w: way_work, opts: &extract_opts) -> Option<admin_level> {
  if w.coords.is_empty() {
    return None;
  }

  // streets are always lines — never polygon, even if the ring is closed
  let geom = Geometry::LineString(LineString(w.coords));

  let identity = identity_of(opts, level::street, None, &geom, &w.names)?;
  Some(admin_level {
    id: admin_level_id::from_way(w.way_id),
    level: level::street,
    name: identity.name,
    country_iso_code: identity.country_iso_code,
    post_code: w.post_code,
    wkb: geom.into(),
  })
}
