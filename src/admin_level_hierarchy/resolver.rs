use geo::{Area, BoundingRect, Centroid, Geometry, LineString};
use rstar::{AABB, RTree};
use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::thread;

use super::entity::hierarchy_edges;
use super::repository::{self, admin_levels_hierarchy};
use crate::admin_level::geometry::admin_geometry;
use crate::admin_level::level;
use crate::admin_level::point_in_polygon::{
  classify_in_polygons, extract_polygons, point_class, polygon_entry, spatial_entry,
};
use crate::admin_level::repository as admin_level_repository;
use crate::admin_level::repository::admin_level_geom_row;
use crate::database::table;
use crate::progress_report;

const BATCH_SIZE: usize = 10_000;
const READ_SIZE: usize = 5_000;
const MAX_WORKERS: usize = 8;
// how much of an area has to sit inside another for it to be a parent: a tenth is enough to
// straddle a more general area, while nesting under a peer of the same level asks for most of it,
// or a sloppily drawn neighbourhood would swallow the one beside it
const STRADDLE_FRACTION: f64 = 0.10;
const NESTING_FRACTION: f64 = 0.50;
const GRID_SIDE: usize = 8;

struct ancestor_entry {
  id: i64,
  admin_level: level,
  bbox: Option<[f64; 4]>,
  polys: Vec<polygon_entry>,
  samples: Vec<[f64; 2]>,
  area: f64,
  cx: f64,
  cy: f64,
  ancestors: HashSet<i64>,
}

struct sink<'a, F: Fn(progress_report)> {
  conn: &'a Connection,
  total: u64,
  processed: u64,
  batch: Vec<hierarchy_edges>,
  progress: &'a F,
}

impl<F: Fn(progress_report)> sink<'_, F> {
  fn push(&mut self, row: hierarchy_edges) {
    self.batch.push(row);
    self.processed += 1;
  }

  fn extend(&mut self, rows: Vec<hierarchy_edges>) {
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
    admin_levels_hierarchy::create_indexes(conn);
    return;
  }

  let raw = admin_level_repository::load_all_below_street(conn);
  let entries: Vec<ancestor_entry> = raw.iter().map(parse_entry).collect();
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
  let mut entries = entries;
  for indices in by_level.values() {
    // the geometry runs in parallel against the entries as they are; the reduction that follows
    // reads the finished ancestors of a parent, so it runs alone, largest area first
    let qualified = resolve_level(&entries, indices, &tree, n_workers);
    for (position, &idx) in indices.iter().enumerate() {
      let parents = reduce_to_parents(&qualified[position], &entries);
      let mut ancestors: HashSet<i64> = HashSet::new();
      for &p in &parents {
        ancestors.insert(entries[p].id);
        ancestors.extend(entries[p].ancestors.iter().copied());
      }
      entries[idx].ancestors = ancestors;
      sink.push(hierarchy_edges {
        admin_level_id: entries[idx].id,
        parents: parents.iter().map(|&p| entries[p].id).collect(),
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
  admin_levels_hierarchy::create_indexes(conn);
}

fn worker_count() -> usize {
  thread::available_parallelism()
    .map(NonZeroUsize::get)
    .unwrap_or(4)
    .min(MAX_WORKERS)
}

// largest area first within a level, then by id, so that the order never depends on the load
fn indices_by_level(entries: &[ancestor_entry]) -> BTreeMap<level, Vec<usize>> {
  let mut by_level: BTreeMap<level, Vec<usize>> = BTreeMap::new();
  for (idx, e) in entries.iter().enumerate() {
    by_level.entry(e.admin_level).or_default().push(idx);
  }
  for indices in by_level.values_mut() {
    indices.sort_by(|&a, &b| {
      entries[b]
        .area
        .partial_cmp(&entries[a].area)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then(entries[a].id.cmp(&entries[b].id))
    });
  }
  by_level
}

// the candidates each area qualifies under, by position in `indices`: the workers answer in
// whatever order they finish, so the position is what puts the answer back in place
fn resolve_level(
  entries: &[ancestor_entry],
  indices: &[usize],
  tree: &RTree<spatial_entry>,
  n_workers: usize,
) -> Vec<Vec<usize>> {
  let chunk_size = indices.len().div_ceil(n_workers).max(1);
  let (tx, rx) = mpsc::channel::<(usize, Vec<usize>)>();
  let mut results: Vec<Vec<usize>> = vec![Vec::new(); indices.len()];

  thread::scope(|s| {
    for (chunk_index, chunk) in indices.chunks(chunk_size).enumerate() {
      let tx = tx.clone();
      let base = chunk_index * chunk_size;
      s.spawn(move || {
        for (offset, &idx) in chunk.iter().enumerate() {
          tx.send((base + offset, qualifying_candidates(&entries[idx], tree, entries)))
            .ok();
        }
      });
    }
    drop(tx);
    for (position, qualifying) in rx {
      results[position] = qualifying;
    }
  });
  results
}

fn street_lines_as_entries(row: &admin_level_geom_row) -> Vec<ancestor_entry> {
  match row.wkb.as_ref().map(admin_geometry::geometry) {
    Some(Geometry::MultiLineString(lines)) if lines.0.len() > 1 => lines
      .0
      .iter()
      .map(|line| {
        parse_entry(&admin_level_geom_row {
          id: row.id,
          admin_level: row.admin_level,
          wkb: Some(admin_geometry(Geometry::LineString(line.clone()))),
        })
      })
      .collect(),
    _ => vec![parse_entry(row)],
  }
}

fn resolve_street_rows(
  conn: &Connection,
  ids: &[i64],
  tree: &RTree<spatial_entry>,
  entries: &[ancestor_entry],
) -> Vec<hierarchy_edges> {
  admin_level_repository::load_by_ids(conn, ids)
    .iter()
    .map(|db_row| {
      let qualifying: BTreeSet<usize> = street_lines_as_entries(db_row)
        .iter()
        .flat_map(|line| qualifying_candidates(line, tree, entries))
        .collect();
      let qualifying: Vec<usize> = qualifying.into_iter().collect();
      hierarchy_edges {
        admin_level_id: db_row.id,
        parents: reduce_to_parents(&qualifying, entries)
          .iter()
          .map(|&p| entries[p].id)
          .collect(),
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
  let (tx, rx) = mpsc::channel::<Vec<hierarchy_edges>>();
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
  tx: &mpsc::Sender<Vec<hierarchy_edges>>,
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
  let polys = geometry
    .as_ref()
    .map(|g| extract_polygons(g.clone()))
    .unwrap_or_default();
  let samples = geometry
    .as_ref()
    .map(|g| sample_points(g, &polys, bbox, cx, cy))
    .unwrap_or_default();
  ancestor_entry {
    id: row.id,
    admin_level: row.admin_level,
    bbox,
    polys,
    samples,
    area,
    cx,
    cy,
    ancestors: HashSet::new(),
  }
}

// where an area is asked about: the interior of a polygon on a grid, the vertices and the middle
// of each segment of a line. a border two areas share runs through the vertices of both, which is
// why a line is asked about its midpoints too, and an area only about points strictly inside it
fn sample_points(
  geometry: &Geometry<f64>,
  polys: &[polygon_entry],
  bbox: Option<[f64; 4]>,
  cx: f64,
  cy: f64,
) -> Vec<[f64; 2]> {
  if polys.is_empty() {
    let mut samples = Vec::new();
    line_samples(geometry, &mut samples);
    return samples;
  }
  let Some([min_x, min_y, max_x, max_y]) = bbox else {
    return vec![[cx, cy]];
  };
  let mut samples = Vec::with_capacity(GRID_SIDE * GRID_SIDE + 1);
  for column in 0..GRID_SIDE {
    for row in 0..GRID_SIDE {
      let step = |min: f64, max: f64, at: usize| {
        min + (max - min) * (at as f64 + 0.5) / GRID_SIDE as f64
      };
      let (x, y) = (step(min_x, max_x, column), step(min_y, max_y, row));
      if classify_in_polygons(x, y, polys) == point_class::inside {
        samples.push([x, y]);
      }
    }
  }
  if classify_in_polygons(cx, cy, polys) == point_class::inside {
    samples.push([cx, cy]);
  }
  if samples.is_empty() {
    samples.push([cx, cy]);
  }
  samples
}

fn line_samples(geometry: &Geometry<f64>, out: &mut Vec<[f64; 2]>) {
  match geometry {
    Geometry::LineString(ls) => push_line_samples(ls, out),
    Geometry::MultiLineString(mls) => mls.0.iter().for_each(|ls| push_line_samples(ls, out)),
    Geometry::Polygon(p) => push_line_samples(p.exterior(), out),
    Geometry::MultiPolygon(mp) => mp.0.iter().for_each(|p| push_line_samples(p.exterior(), out)),
    Geometry::Point(p) => out.push([p.x(), p.y()]),
    Geometry::GeometryCollection(gc) => gc.0.iter().for_each(|g| line_samples(g, out)),
    _ => {}
  }
}

fn push_line_samples(ls: &LineString<f64>, out: &mut Vec<[f64; 2]>) {
  let mut previous: Option<[f64; 2]> = None;
  for coord in &ls.0 {
    let point = [coord.x, coord.y];
    if let Some(before) = previous {
      out.push([
        (before[0] + point[0]) / 2.0,
        (before[1] + point[1]) / 2.0,
      ]);
    }
    out.push(point);
    previous = Some(point);
  }
}

// every area the child sits in, at any level: an area it entered, plus one it only runs along the
// border of when that one is more specific than everything it entered, which is the street traced
// over the line two neighbourhoods share.
// the candidates are tried from the most specific outward and an area already above one that
// qualified is never measured: the reduction would drop it anyway, and the ring of a country is
// the expensive one to walk
fn qualifying_candidates(
  e: &ancestor_entry,
  tree: &RTree<spatial_entry>,
  entries: &[ancestor_entry],
) -> Vec<usize> {
  let mut candidates: Vec<usize> = tree
    .locate_in_envelope_intersecting(&envelope_of(e))
    .map(|se| se.idx)
    .filter(|&idx| {
      let c = &entries[idx];
      c.id != e.id
        && !c.polys.is_empty()
        && (c.admin_level < e.admin_level
          || (c.admin_level == e.admin_level && c.area > e.area))
    })
    .collect();
  candidates.sort_by(|&a, &b| {
    entries[b]
      .admin_level
      .cmp(&entries[a].admin_level)
      .then(
        entries[a]
          .area
          .partial_cmp(&entries[b].area)
          .unwrap_or(std::cmp::Ordering::Equal),
      )
      .then(entries[a].id.cmp(&entries[b].id))
  });

  let mut inside: Vec<usize> = Vec::new();
  let mut on_edge: Vec<usize> = Vec::new();
  let mut covered: HashSet<i64> = HashSet::new();
  for idx in candidates {
    let c = &entries[idx];
    if covered.contains(&c.id) {
      continue;
    }
    match containment(e, c) {
      point_class::inside => {
        covered.extend(c.ancestors.iter().copied());
        inside.push(idx);
      }
      point_class::on_edge => on_edge.push(idx),
      point_class::outside => {}
    }
  }
  let mut qualifying = inside;
  // only a street is promoted by a border it never crosses: two areas that share a boundary are
  // neighbours, and reading that border as containment would nest every city in the one beside it
  if e.admin_level == level::street {
    let deepest_entered = qualifying.iter().map(|&idx| entries[idx].admin_level).max();
    for idx in on_edge {
      if deepest_entered.is_none_or(|deepest| entries[idx].admin_level > deepest) {
        qualifying.push(idx);
      }
    }
  }
  qualifying
}

// an area that another qualifying area already hangs from is not a parent: the street inside a
// neighbourhood hangs from the neighbourhood, not from its city as well
fn reduce_to_parents(qualifying: &[usize], entries: &[ancestor_entry]) -> Vec<usize> {
  let mut parents: Vec<usize> = qualifying
    .iter()
    .copied()
    .filter(|&idx| {
      let id = entries[idx].id;
      !qualifying
        .iter()
        .any(|&other| other != idx && entries[other].ancestors.contains(&id))
    })
    .collect();
  parents.sort_by_key(|&idx| entries[idx].id);
  parents.dedup_by_key(|&mut idx| entries[idx].id);
  parents
}

fn envelope_of(e: &ancestor_entry) -> AABB<[f64; 2]> {
  match e.bbox {
    Some([min_x, min_y, max_x, max_y]) => AABB::from_corners([min_x, min_y], [max_x, max_y]),
    None => AABB::from_point([e.cx, e.cy]),
  }
}

// a street belongs to every area it enters, so one point inside is enough; an area belongs where
// enough of it falls, whether it closed into a polygon or was clipped into a line by the extract
fn containment(child: &ancestor_entry, candidate: &ancestor_entry) -> point_class {
  let fraction = if candidate.admin_level == child.admin_level {
    NESTING_FRACTION
  } else {
    STRADDLE_FRACTION
  };
  let needed = if child.admin_level == level::street {
    1
  } else {
    ((child.samples.len() as f64) * fraction).ceil().max(1.0) as usize
  };
  let Some([min_x, min_y, max_x, max_y]) = candidate.bbox else {
    return point_class::outside;
  };
  let mut hits = 0;
  let mut touched_edge = false;
  let mut left = child.samples.len();
  for sample in &child.samples {
    let [x, y] = *sample;
    // the box first: walking the ring of a state for a point that is plainly outside it is the
    // one cost this pass cannot afford
    if x >= min_x && x <= max_x && y >= min_y && y <= max_y {
      match classify_in_polygons(x, y, &candidate.polys) {
        point_class::inside => {
          hits += 1;
          if hits >= needed {
            return point_class::inside;
          }
        }
        point_class::on_edge => touched_edge = true,
        point_class::outside => {}
      }
    }
    left -= 1;
    if hits + left < needed {
      break;
    }
  }
  if touched_edge {
    point_class::on_edge
  } else {
    point_class::outside
  }
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

