use geo::{Coord, Geometry, LineString};
use rusqlite::Connection;

struct way_work {
  way_id: u64,
  name: String,
  post_code: Option<String>,
  coords: Vec<Coord<f64>>,
}

pub fn run(
  conn: &Connection,
  rules: &[super::extraction_rules],
  name_priority: &[&str],
  progress: impl Fn(super::progress_report),
) {
  let level = super::osm_admin_level::street as u8;
  let (_, exclude) = super::resolve_rules(12, rules);
  let candidate_ids = crate::database::osm_ways::remaining_ids_by_tags(conn, level, exclude);

  let total = candidate_ids.len() as u64;
  progress(super::progress_report {
    total: Some(total),
    processed: 0,
  });
  if candidate_ids.is_empty() {
    return;
  }

  // the per-item geometry work (line assembly) is far cheaper than the
  // single-connection db read/write that bounds this stage, so processing
  // sequentially is as fast as a worker pool without the channel overhead
  let mut processed: u64 = 0;
  for chunk in candidate_ids.chunks(super::CHUNK_SIZE) {
    let works = load_chunk(conn, chunk, name_priority);
    let mut batch: Vec<crate::database::admin_levels::admin_levels> = Vec::new();
    for w in works {
      if let Some(row) = process_one_way(w.way_id, &w.coords, &w.name, w.post_code.as_deref()) {
        batch.push(row);
      }
    }
    processed += crate::database::admin_levels::batch_upsert(conn, &batch) as u64;
    progress(super::progress_report {
      total: Some(total),
      processed,
    });
  }
}

fn load_chunk(conn: &Connection, chunk: &[u64], name_priority: &[&str]) -> Vec<way_work> {
  let raw_rows = crate::database::osm_ways::way_coords_chunk(conn, chunk, name_priority);

  type way_name_tuple = (String, Option<String>);

  let mut way_order: Vec<u64> = Vec::new();
  let mut way_names: std::collections::HashMap<u64, way_name_tuple> =
    std::collections::HashMap::new();
  let mut way_coords: std::collections::HashMap<u64, Vec<Coord<f64>>> =
    std::collections::HashMap::new();

  for row in &raw_rows {
    if let std::collections::hash_map::Entry::Vacant(e) = way_names.entry(row.way_id) {
      way_order.push(row.way_id);
      e.insert((row.way_name.clone(), row.post_code.clone()));
      way_coords.insert(row.way_id, Vec::new());
    }
    way_coords.get_mut(&row.way_id).unwrap().push(Coord {
      x: row.lon,
      y: row.lat,
    });
  }

  let mut works = Vec::with_capacity(way_order.len());
  for way_id in way_order {
    let (name, post_code) = way_names.remove(&way_id).unwrap();
    let coords = way_coords.remove(&way_id).unwrap();
    works.push(way_work {
      way_id,
      name,
      post_code,
      coords,
    });
  }
  works
}

fn process_one_way(
  way_id: u64,
  coords: &[Coord<f64>],
  name: &str,
  post_code: Option<&str>,
) -> Option<crate::database::admin_levels::admin_levels> {
  if coords.is_empty() {
    return None;
  }

  // streets are always lines — never polygon, even if the ring is closed
  let ls = LineString(coords.to_vec());

  Some(crate::database::admin_levels::admin_levels {
    relation_id: None,
    way_id: Some(way_id),
    admin_level: super::osm_admin_level::street as u8,
    name: name.to_owned(),
    country_iso_code: None,
    post_code: post_code.map(str::to_owned),
    wkb: Geometry::LineString(ls).into(),
  })
}
