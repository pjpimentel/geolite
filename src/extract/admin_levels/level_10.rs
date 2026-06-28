use geo::{Coord, Geometry, LineString, MultiPolygon, Polygon, Winding};
use rusqlite::Connection;

struct way_meta {
  name: String,
  post_code: Option<String>,
}

struct way_work {
  way_id: u64,
  meta: way_meta,
  coords: Vec<Coord<f64>>,
  admin_level: super::osm_admin_level,
}

pub fn run(
  conn: &Connection,
  rules: &[super::extraction_rules],
  name_priority: &[&str],
  progress: impl Fn(super::progress_report),
) {
  let level = super::osm_admin_level::neighborhood as u8;
  let (include, _) = super::resolve_rules(10, rules);
  let mut candidate_ids: Vec<u64> = Vec::new();
  for filter in include {
    let ids =
      crate::database::osm_ways::remaining_ids_by_tags(conn, level, std::slice::from_ref(filter));
    candidate_ids.extend(ids);
  }
  candidate_ids.sort_unstable();
  candidate_ids.dedup();

  let total = candidate_ids.len() as u64;
  progress(super::progress_report {
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
  for chunk in candidate_ids.chunks(super::CHUNK_SIZE) {
    let works = load_chunk(
      conn,
      chunk,
      super::osm_admin_level::neighborhood,
      name_priority,
    );
    let mut batch: Vec<crate::database::admin_levels::admin_levels> = Vec::new();
    for w in works {
      if let Some(row) = process_one_way(w) {
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

fn load_chunk(
  conn: &Connection,
  chunk: &[u64],
  admin_level: super::osm_admin_level,
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
      admin_level,
    });
  }
  works
}

fn process_one_way(w: way_work) -> Option<crate::database::admin_levels::admin_levels> {
  if w.coords.is_empty() {
    return None;
  }

  let ls = LineString(w.coords);
  let geom: Geometry<f64> =
    if ls.0.len() >= 4 && super::approx_eq(ls.0[0], *ls.0.last().unwrap()) {
      let mut ring = ls;
      // spatialite st_buildarea reverses ccw rings to cw — replicate that behavior
      ring.make_cw_winding();
      Geometry::MultiPolygon(MultiPolygon(vec![Polygon::new(ring, vec![])]))
    } else {
      Geometry::LineString(ls)
    };

  Some(crate::database::admin_levels::admin_levels {
    relation_id: None,
    way_id: Some(w.way_id),
    admin_level: w.admin_level as u8,
    name: w.meta.name,
    country_iso_code: None,
    post_code: w.meta.post_code,
    wkb: geom.into(),
  })
}
