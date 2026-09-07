use geo::{Coord, Geometry, LineString, MultiLineString, MultiPolygon, Polygon, Winding};
use rusqlite::Connection;
use std::sync::{Arc, Mutex, mpsc};

use super::entity::admin_level;
use super::extract::{CHUNK_SIZE, progress_report};
use super::geometry::{approx_eq, assemble_rings};
use super::scale::level;

struct rel_meta {
  name: String,
  country_iso_code: Option<String>,
  post_code: Option<String>,
}

struct rel_work {
  relation_id: u64,
  meta: rel_meta,
  ways: Vec<LineString<f64>>,
}

pub(super) fn run_with_ids(
  conn: &Connection,
  ids: Vec<u64>,
  level: level,
  threads: usize,
  name_priority: &[&str],
  mut progress: impl FnMut(progress_report),
) {
  let total = ids.len() as u64;
  progress(progress_report {
    total: Some(total),
    processed: 0,
  });
  if ids.is_empty() {
    return;
  }

  let (work_tx, work_rx) = mpsc::channel::<rel_work>();
  let work_rx = Arc::new(Mutex::new(work_rx));
  let (result_tx, result_rx) = mpsc::channel::<Option<admin_level>>();

  std::thread::scope(|s| {
    for _ in 0..threads {
      let rx = work_rx.clone();
      let tx = result_tx.clone();
      s.spawn(move || worker(&rx, &tx, level));
    }
    drop(result_tx);

    let mut processed: u64 = 0;

    for chunk in ids.chunks(CHUNK_SIZE) {
      let dispatched = load_and_send(conn, chunk, name_priority, &work_tx);
      let batch = collect_batch(&result_rx, dispatched);

      processed += super::repository::batch_upsert(conn, &batch) as u64;
      progress(progress_report {
        total: Some(total),
        processed,
      });
    }

    drop(work_tx);
  });
}

fn worker(
  rx: &Mutex<mpsc::Receiver<rel_work>>,
  tx: &mpsc::Sender<Option<admin_level>>,
  level: level,
) {
  loop {
    // the lock must be released before the relation is processed, or the workers serialize
    let item = { rx.lock().unwrap().recv() };
    match item {
      Ok(w) => {
        tx.send(process_one_relation(w.relation_id, &w.meta, &w.ways, level))
          .ok();
      }
      Err(_) => break,
    }
  }
}

fn collect_batch(rx: &mpsc::Receiver<Option<admin_level>>, dispatched: usize) -> Vec<admin_level> {
  let mut batch: Vec<admin_level> = Vec::new();
  for _ in 0..dispatched {
    if let Ok(Some(row)) = rx.recv() {
      batch.push(row);
    }
  }
  batch
}

fn load_and_send(
  conn: &Connection,
  chunk: &[u64],
  name_priority: &[&str],
  tx: &mpsc::Sender<rel_work>,
) -> usize {
  let raw_rows = crate::database::osm_relations::relation_coords_chunk(conn, chunk, name_priority);

  let mut relation_order: Vec<u64> = Vec::new();
  let mut relation_metas: std::collections::HashMap<u64, rel_meta> =
    std::collections::HashMap::new();
  let mut way_builders: std::collections::HashMap<
    u64,
    std::collections::BTreeMap<String, Vec<Coord<f64>>>,
  > = std::collections::HashMap::new();

  for row in &raw_rows {
    if let std::collections::hash_map::Entry::Vacant(e) = relation_metas.entry(row.relation_id) {
      relation_order.push(row.relation_id);
      e.insert(rel_meta {
        name: row.relation_name.clone(),
        country_iso_code: row.country_iso_code.clone(),
        post_code: row.post_code.clone(),
      });
      way_builders.insert(row.relation_id, std::collections::BTreeMap::new());
    }
    let builder = way_builders.get_mut(&row.relation_id).unwrap();
    let way_key = format!("{:010}_{}", row.way_order, row.way_id);
    builder.entry(way_key).or_default().push(Coord {
      x: row.lon,
      y: row.lat,
    });
  }

  let count = relation_order.len();
  for relation_id in relation_order {
    let meta = relation_metas.remove(&relation_id).unwrap();
    let ways: Vec<LineString<f64>> = way_builders
      .remove(&relation_id)
      .unwrap()
      .into_values()
      .map(LineString)
      .collect();
    tx.send(rel_work {
      relation_id,
      meta,
      ways,
    })
    .ok();
  }
  count
}

fn process_one_relation(
  relation_id: u64,
  meta: &rel_meta,
  ways: &[LineString<f64>],
  level: level,
) -> Option<admin_level> {
  if ways.iter().all(|ls| ls.0.is_empty()) {
    return None;
  }

  let rings = assemble_rings(ways);
  let polygon_rings: Vec<LineString<f64>> = rings
    .into_iter()
    .filter(|r| r.0.len() >= 4 && approx_eq(r.0[0], *r.0.last().unwrap()))
    .collect();

  let geom: Geometry<f64> = if !polygon_rings.is_empty() {
    let polygons: Vec<Polygon<f64>> = polygon_rings
      .into_iter()
      .map(|mut ls| {
        // spatialite st_buildarea reverses ccw rings to cw — replicate that behavior
        ls.make_cw_winding();
        Polygon::new(ls, vec![])
      })
      .collect();
    Geometry::MultiPolygon(MultiPolygon(polygons))
  } else {
    Geometry::MultiLineString(MultiLineString(ways.to_vec()))
  };

  Some(admin_level {
    relation_id: Some(relation_id),
    way_id: None,
    level,
    name: meta.name.clone(),
    country_iso_code: meta.country_iso_code.clone(),
    post_code: meta.post_code.clone(),
    wkb: geom.into(),
  })
}

#[cfg(test)]
#[path = "relations.test.rs"]
mod tests;
