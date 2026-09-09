use geo::{Area, BoundingRect, Centroid, Geometry};
use rstar::{AABB, RTree, RTreeObject};
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::thread;

use super::entity::{encode_chain, hierarchy_row};
use super::{label, repository};
use crate::domain::admin_level::level;
use crate::domain::admin_level::repository as admin_level_repository;
use crate::domain::admin_level::repository::admin_level_geom_row;

pub struct progress_report {
  pub total: Option<u64>,
  pub processed: u64,
}

const BATCH_SIZE: usize = 10_000;
const READ_SIZE: usize = 5_000;
const MAX_WORKERS: usize = 8;

struct spatial_entry {
  idx: usize,
  envelope: AABB<[f64; 2]>,
}

impl RTreeObject for spatial_entry {
  type Envelope = AABB<[f64; 2]>;
  fn envelope(&self) -> Self::Envelope {
    self.envelope
  }
}

struct polygon_entry {
  exterior: Vec<[f64; 2]>,
  interiors: Vec<Vec<[f64; 2]>>,
}

struct ancestor_entry {
  id: i64,
  admin_level: level,
  name: String,
  bbox: Option<[f64; 4]>,
  polys: Vec<polygon_entry>,
  area: f64,
  cx: f64,
  cy: f64,
  ancestor_ids: Vec<i64>,
  user_friendly_name: String,
  own_post_code: Option<String>,
}

struct sink<'a, F: Fn(progress_report)> {
  conn: &'a Connection,
  total: u64,
  processed: u64,
  batch: Vec<hierarchy_row>,
  progress: &'a F,
}

impl<F: Fn(progress_report)> sink<'_, F> {
  fn push(&mut self, row: hierarchy_row) {
    self.batch.push(row);
    self.processed += 1;
  }

  fn extend(&mut self, rows: Vec<hierarchy_row>) {
    self.processed += rows.len() as u64;
    self.batch.extend(rows);
    self.flush_if_full();
  }

  fn flush_if_full(&mut self) {
    if self.batch.len() >= BATCH_SIZE {
      self.flush();
    }
  }

  fn flush(&mut self) {
    if self.batch.is_empty() {
      return;
    }
    repository::batch_insert(self.conn, &self.batch);
    self.batch.clear();
    (self.progress)(progress_report {
      total: Some(self.total),
      processed: self.processed,
    });
  }
}

pub fn run(conn: &Connection, progress: impl Fn(progress_report)) {
  let total = repository::pending_total(conn) as u64;
  progress(progress_report {
    total: Some(total),
    processed: 0,
  });

  if total == 0 {
    return;
  }

  let raw = admin_level_repository::load_all_below_street(conn);
  let mut entries: Vec<ancestor_entry> = raw.iter().map(parse_entry).collect();
  let by_level = indices_by_level(&entries);
  let tree = build_rtree(&entries);
  let n_workers = worker_count();
  let mut sink = sink {
    conn,
    total,
    processed: 0,
    batch: Vec::new(),
    progress: &progress,
  };

  // levels ascend so that every parent is final before its children look it up
  for indices in by_level.values() {
    for (idx, ancestor_ids, user_friendly_name) in resolve_level(&entries, indices, &tree, n_workers) {
      entries[idx].ancestor_ids = ancestor_ids;
      entries[idx].user_friendly_name = user_friendly_name;
    }
    propagate_chains(&mut entries, indices);
    for &idx in indices {
      sink.push(hierarchy_row {
        admin_level_id: entries[idx].id,
        ancestor_ids: encode_chain(&entries[idx].ancestor_ids),
        user_friendly_name: entries[idx].user_friendly_name.clone(),
      });
    }
    sink.flush_if_full();
  }
  sink.flush();

  let street_ids = repository::pending_street_ids(conn);
  // conn.path() is Some("") for `:memory:`, which a worker could not reopen
  match conn.path().filter(|p| !p.is_empty()) {
    Some(path) => run_streets_parallel(path.to_owned(), &tree, &entries, &street_ids, n_workers, &mut sink),
    None => run_streets_sequential(&tree, &entries, &street_ids, &mut sink),
  }
  sink.flush();
}

fn worker_count() -> usize {
  thread::available_parallelism()
    .map(NonZeroUsize::get)
    .unwrap_or(4)
    .min(MAX_WORKERS)
}

fn indices_by_level(entries: &[ancestor_entry]) -> BTreeMap<level, Vec<usize>> {
  let mut by_level: BTreeMap<level, Vec<usize>> = BTreeMap::new();
  for (idx, e) in entries.iter().enumerate() {
    by_level.entry(e.admin_level).or_default().push(idx);
  }
  // largest first, so that the peer pass finds an enclosing peer already finished
  for indices in by_level.values_mut() {
    indices.sort_by(|&a, &b| {
      entries[b]
        .area
        .partial_cmp(&entries[a].area)
        .unwrap_or(std::cmp::Ordering::Equal)
    });
  }
  by_level
}

fn resolve_level(
  entries: &[ancestor_entry],
  indices: &[usize],
  tree: &RTree<spatial_entry>,
  n_workers: usize,
) -> Vec<(usize, Vec<i64>, String)> {
  let chunk_size = indices.len().div_ceil(n_workers).max(1);
  let (tx, rx) = mpsc::channel::<(usize, Vec<i64>, String)>();
  let mut results: Vec<(usize, Vec<i64>, String)> = Vec::with_capacity(indices.len());

  thread::scope(|s| {
    for chunk in indices.chunks(chunk_size) {
      let tx = tx.clone();
      s.spawn(move || {
        for &idx in chunk {
          let (ancestor_ids, user_friendly_name) = resolve_hierarchy(&entries[idx], tree, entries);
          tx.send((idx, ancestor_ids, user_friendly_name)).ok();
        }
      });
    }
    drop(tx);
    for item in rx {
      results.push(item);
    }
  });
  results
}

// peers of one level resolve in parallel against entries in their initial state, so a chain
// a → b → c between peers comes back truncated (b's label is still "b" when c reads it). the
// indices are in descending area order, so re-applying the first ancestor's finished chain
// propagates it transitively.
fn propagate_chains(entries: &mut [ancestor_entry], indices: &[usize]) {
  for &idx in indices {
    let Some(parent_id) = entries[idx].ancestor_ids.first().copied() else {
      continue;
    };
    let Some(parent_idx) = entries.iter().position(|e| e.id == parent_id) else {
      continue;
    };
    let new_ancestors: Vec<i64> = std::iter::once(parent_id)
      .chain(entries[parent_idx].ancestor_ids.iter().copied())
      .collect();
    let new_label = label::nested(
      &entries[idx].name,
      &entries[parent_idx].user_friendly_name,
      entries[idx].own_post_code.as_deref(),
    );
    entries[idx].ancestor_ids = new_ancestors;
    entries[idx].user_friendly_name = new_label;
  }
}

fn resolve_street_rows(
  conn: &Connection,
  ids: &[i64],
  tree: &RTree<spatial_entry>,
  entries: &[ancestor_entry],
) -> Vec<hierarchy_row> {
  admin_level_repository::load_by_ids(conn, ids)
    .iter()
    .map(|db_row| {
      let e = parse_entry(db_row);
      let (ancestor_ids, user_friendly_name) = resolve_hierarchy(&e, tree, entries);
      hierarchy_row {
        admin_level_id: e.id,
        ancestor_ids: encode_chain(&ancestor_ids),
        user_friendly_name,
      }
    })
    .collect()
}

fn run_streets_parallel<F: Fn(progress_report)>(
  path: String,
  tree: &RTree<spatial_entry>,
  entries: &[ancestor_entry],
  street_ids: &[i64],
  n_workers: usize,
  sink: &mut sink<F>,
) {
  let (tx, rx) = mpsc::channel::<Vec<hierarchy_row>>();
  let chunk_size = street_ids.len().div_ceil(n_workers).max(1);

  thread::scope(|s| {
    for id_chunk in street_ids.chunks(chunk_size) {
      let tx = tx.clone();
      let path = path.clone();
      s.spawn(move || scan_streets(&path, id_chunk, tree, entries, &tx));
    }
    drop(tx);

    for rows in rx {
      sink.extend(rows);
    }
  });
}

fn scan_streets(
  path: &str,
  ids: &[i64],
  tree: &RTree<spatial_entry>,
  entries: &[ancestor_entry],
  tx: &mpsc::Sender<Vec<hierarchy_row>>,
) {
  let Ok(reader) = Connection::open_with_flags(
    path,
    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
  ) else {
    return;
  };
  for sub_chunk in ids.chunks(READ_SIZE) {
    tx.send(resolve_street_rows(&reader, sub_chunk, tree, entries))
      .ok();
  }
}

fn run_streets_sequential<F: Fn(progress_report)>(
  tree: &RTree<spatial_entry>,
  entries: &[ancestor_entry],
  street_ids: &[i64],
  sink: &mut sink<F>,
) {
  for chunk in street_ids.chunks(READ_SIZE) {
    let rows = resolve_street_rows(sink.conn, chunk, tree, entries);
    sink.extend(rows);
  }
}

fn parse_entry(row: &admin_level_geom_row) -> ancestor_entry {
  let geometry = row.wkb.as_ref().map(|g| g.geometry().clone());
  let (cx, cy) = geometry
    .as_ref()
    .and_then(|g| g.centroid())
    .map(|p| (p.x(), p.y()))
    .unwrap_or((0.0, 0.0));
  let area = geometry.as_ref().map(|g| g.unsigned_area()).unwrap_or(0.0);
  let bbox = geometry
    .as_ref()
    .and_then(|g| g.bounding_rect())
    .map(|r| [r.min().x, r.min().y, r.max().x, r.max().y]);
  let polys = geometry.map(extract_polygons).unwrap_or_default();
  let own_post_code = row.post_code.clone();
  let user_friendly_name = label::root(&row.name, own_post_code.as_deref());
  ancestor_entry {
    id: row.id,
    admin_level: row.admin_level,
    name: row.name.clone(),
    bbox,
    polys,
    area,
    cx,
    cy,
    ancestor_ids: vec![],
    user_friendly_name,
    own_post_code,
  }
}

fn extract_polygons(geometry: Geometry<f64>) -> Vec<polygon_entry> {
  match geometry {
    Geometry::Polygon(p) => vec![polygon_to_entry(p)],
    Geometry::MultiPolygon(mp) => mp.0.into_iter().map(polygon_to_entry).collect(),
    _ => vec![],
  }
}

fn polygon_to_entry(p: geo::Polygon<f64>) -> polygon_entry {
  let (exterior, interiors) = p.into_inner();
  polygon_entry {
    exterior: exterior.0.into_iter().map(|c| [c.x, c.y]).collect(),
    interiors: interiors
      .into_iter()
      .map(|ring| ring.0.into_iter().map(|c| [c.x, c.y]).collect())
      .collect(),
  }
}

fn resolve_hierarchy(
  e: &ancestor_entry,
  tree: &RTree<spatial_entry>,
  entries: &[ancestor_entry],
) -> (Vec<i64>, String) {
  let by_level = candidates_by_level(e, tree, entries);
  match smallest_enclosing(e, &by_level, entries) {
    None => (vec![], label::root(&e.name, e.own_post_code.as_deref())),
    Some(idx) => {
      let p = &entries[idx];
      let mut ancestor_ids = vec![p.id];
      ancestor_ids.extend_from_slice(&p.ancestor_ids);
      (
        ancestor_ids,
        label::nested(&e.name, &p.user_friendly_name, e.own_post_code.as_deref()),
      )
    }
  }
}

// every area whose box covers the centroid and that could contain this one: a lower level, or
// the same level with a larger area
fn candidates_by_level(
  e: &ancestor_entry,
  tree: &RTree<spatial_entry>,
  entries: &[ancestor_entry],
) -> BTreeMap<level, Vec<usize>> {
  let mut by_level: BTreeMap<level, Vec<usize>> = BTreeMap::new();
  for se in tree.locate_in_envelope_intersecting(&AABB::from_point([e.cx, e.cy])) {
    let c = &entries[se.idx];
    let could_contain = c.id != e.id
      && (c.admin_level < e.admin_level
        || (c.admin_level == e.admin_level && c.area > e.area));
    if could_contain {
      by_level.entry(c.admin_level).or_default().push(se.idx);
    }
  }
  by_level
}

// the most specific level with a polygon around the centroid, and within it the smallest area
fn smallest_enclosing(
  e: &ancestor_entry,
  by_level: &BTreeMap<level, Vec<usize>>,
  entries: &[ancestor_entry],
) -> Option<usize> {
  for candidates in by_level.values().rev() {
    let smallest = candidates
      .iter()
      .copied()
      .filter(|&idx| point_in_polygons(e.cx, e.cy, &entries[idx].polys))
      .min_by(|&a, &b| {
        entries[a]
          .area
          .partial_cmp(&entries[b].area)
          .unwrap_or(std::cmp::Ordering::Equal)
      });
    if smallest.is_some() {
      return smallest;
    }
  }
  None
}

fn point_in_polygons(px: f64, py: f64, polys: &[polygon_entry]) -> bool {
  polys.iter().any(|poly| {
    point_in_ring(px, py, &poly.exterior)
      && poly
        .interiors
        .iter()
        .all(|hole| !point_in_ring(px, py, hole))
  })
}

fn point_in_ring(px: f64, py: f64, ring: &[[f64; 2]]) -> bool {
  let n = ring.len();
  if n < 3 {
    return false;
  }
  let mut inside = false;
  let mut j = n - 1;
  for i in 0..n {
    let [xi, yi] = ring[i];
    let [xj, yj] = ring[j];
    if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
      inside = !inside;
    }
    j = i;
  }
  inside
}

fn build_rtree(entries: &[ancestor_entry]) -> RTree<spatial_entry> {
  let objects: Vec<spatial_entry> = entries
    .iter()
    .enumerate()
    .filter_map(|(idx, e)| {
      e.bbox.map(|[min_x, min_y, max_x, max_y]| spatial_entry {
        idx,
        envelope: AABB::from_corners([min_x, min_y], [max_x, max_y]),
      })
    })
    .collect();
  RTree::bulk_load(objects)
}

#[cfg(test)]
#[path = "resolver.test.rs"]
mod tests;
