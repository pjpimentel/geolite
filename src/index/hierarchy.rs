use geo::{Area, BoundingRect, Centroid, Geometry};
use rstar::{AABB, RTree, RTreeObject};
use std::collections::BTreeMap;
use std::sync::mpsc;
use std::thread;

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
  admin_level: u8,
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

pub fn run(conn: &rusqlite::Connection, progress: impl Fn(progress_report)) {
  let total = crate::database::admin_levels::pending_total(conn) as u64;
  progress(progress_report {
    total: Some(total),
    processed: 0,
  });

  if total == 0 {
    return;
  }

  let raw = crate::database::admin_levels::load_all_below_street(conn);
  let mut entries: Vec<ancestor_entry> = raw.iter().map(parse_entry).collect();

  let mut by_level: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
  for (idx, e) in entries.iter().enumerate() {
    by_level.entry(e.admin_level).or_default().push(idx);
  }
  for indices in by_level.values_mut() {
    indices.sort_by(|&a, &b| {
      entries[b]
        .area
        .partial_cmp(&entries[a].area)
        .unwrap_or(std::cmp::Ordering::Equal)
    });
  }

  let tree = build_rtree(&entries);

  let n_workers = thread::available_parallelism()
    .map(|n| n.get())
    .unwrap_or(4)
    .min(MAX_WORKERS);

  let mut batch: Vec<crate::database::admin_levels_hierarchy::hierarchy_row> = Vec::new();
  let mut processed = 0u64;

  // process levels ASC so parents are resolved before children
  for indices in by_level.values() {
    let chunk_size = indices.len().div_ceil(n_workers).max(1);
    let (tx, rx) = mpsc::channel::<(usize, Vec<i64>, String)>();
    let mut level_results: Vec<(usize, Vec<i64>, String)> = Vec::with_capacity(indices.len());

    thread::scope(|s| {
      for chunk in indices.chunks(chunk_size) {
        let tx = tx.clone();
        let entries_ref: &[ancestor_entry] = &entries;
        let tree_ref: &RTree<spatial_entry> = &tree;
        s.spawn(move || {
          for &idx in chunk {
            let e = &entries_ref[idx];
            let (ancestor_ids, user_friendly_name) = resolve_hierarchy(
              e.cx,
              e.cy,
              e.id,
              &e.name,
              e.own_post_code.as_deref(),
              e.admin_level,
              e.area,
              tree_ref,
              entries_ref,
            );
            tx.send((idx, ancestor_ids, user_friendly_name)).ok();
          }
        });
      }
      drop(tx);
      for item in rx {
        level_results.push(item);
      }
    });

    for (idx, ancestor_ids, user_friendly_name) in level_results {
      entries[idx].ancestor_ids = ancestor_ids;
      entries[idx].user_friendly_name = user_friendly_name;
    }

    // peers no mesmo admin_level sao resolvidos em paralelo lendo entries em estado
    // inicial — chains a→b→c entre pares saem truncadas (b.ufn = "b" no momento em
    // que c lê). indices esta em ordem DESC de area, entao reaplicamos a chain do
    // primeiro ancestor ja finalizado para propagar transitivamente.
    for &idx in indices.iter() {
      let parent_id = match entries[idx].ancestor_ids.first().copied() {
        Some(id) => id,
        None => continue,
      };
      if let Some(parent_idx) = entries.iter().position(|e| e.id == parent_id) {
        let new_ancestors: Vec<i64> = std::iter::once(parent_id)
          .chain(entries[parent_idx].ancestor_ids.iter().copied())
          .collect();
        let base = format!(
          "{}, {}",
          entries[idx].name, entries[parent_idx].user_friendly_name
        );
        let new_ufn = with_postcode(&base, entries[idx].own_post_code.as_deref());
        entries[idx].ancestor_ids = new_ancestors;
        entries[idx].user_friendly_name = new_ufn;
      }
    }

    for &idx in indices.iter() {
      batch.push(crate::database::admin_levels_hierarchy::hierarchy_row {
        admin_level_id: entries[idx].id,
        ancestor_ids: ids_to_json(&entries[idx].ancestor_ids),
        user_friendly_name: entries[idx].user_friendly_name.clone(),
      });
      processed += 1;
    }

    if batch.len() >= BATCH_SIZE {
      crate::database::admin_levels_hierarchy::batch_insert(conn, &batch);
      batch.clear();
      progress(progress_report {
        total: Some(total),
        processed,
      });
    }
  }

  if !batch.is_empty() {
    crate::database::admin_levels_hierarchy::batch_insert(conn, &batch);
    batch.clear();
    progress(progress_report {
      total: Some(total),
      processed,
    });
  }

  let street_ids = crate::database::admin_levels::pending_street_ids(conn);
  // conn.path() retorna Some("") para `:memory:` — workers nao conseguem reabrir,
  // entao fallback para o caminho sequencial.
  match conn.path().filter(|p| !p.is_empty()) {
    Some(path) => run_streets_parallel(
      conn,
      path.to_owned(),
      &tree,
      &entries,
      &street_ids,
      total,
      &mut processed,
      n_workers,
      &progress,
    ),
    None => run_streets_sequential(
      conn,
      &tree,
      &entries,
      &street_ids,
      total,
      &mut processed,
      &progress,
    ),
  }
}

#[allow(clippy::too_many_arguments)]
fn run_streets_parallel(
  conn: &rusqlite::Connection,
  path: String,
  tree: &RTree<spatial_entry>,
  entries: &[ancestor_entry],
  street_ids: &[i64],
  total: u64,
  processed: &mut u64,
  n_workers: usize,
  progress: &impl Fn(progress_report),
) {
  type row_t = crate::database::admin_levels_hierarchy::hierarchy_row;
  let (tx, rx) = mpsc::channel::<Vec<row_t>>();
  let chunk_size = street_ids.len().div_ceil(n_workers).max(1);

  thread::scope(|s| {
    for id_chunk in street_ids.chunks(chunk_size) {
      let tx = tx.clone();
      let path = path.clone();
      let entries_ref: &[ancestor_entry] = entries;
      let tree_ref: &RTree<spatial_entry> = tree;
      s.spawn(move || {
        let Ok(reader) = rusqlite::Connection::open_with_flags(
          &path,
          rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) else {
          return;
        };
        for sub_chunk in id_chunk.chunks(READ_SIZE) {
          let rows = crate::database::admin_levels::load_by_ids(&reader, sub_chunk);
          let out: Vec<row_t> = rows
            .iter()
            .map(|db_row| {
              let e = parse_entry(db_row);
              let (ancestor_ids, user_friendly_name) = resolve_hierarchy(
                e.cx,
                e.cy,
                e.id,
                &e.name,
                e.own_post_code.as_deref(),
                e.admin_level,
                e.area,
                tree_ref,
                entries_ref,
              );
              row_t {
                admin_level_id: e.id,
                ancestor_ids: ids_to_json(&ancestor_ids),
                user_friendly_name,
              }
            })
            .collect();
          tx.send(out).ok();
        }
      });
    }
    drop(tx);

    let mut batch: Vec<row_t> = Vec::new();
    for sub_batch in rx {
      *processed += sub_batch.len() as u64;
      batch.extend(sub_batch);
      if batch.len() >= BATCH_SIZE {
        crate::database::admin_levels_hierarchy::batch_insert(conn, &batch);
        batch.clear();
        progress(progress_report {
          total: Some(total),
          processed: *processed,
        });
      }
    }
    if !batch.is_empty() {
      crate::database::admin_levels_hierarchy::batch_insert(conn, &batch);
      progress(progress_report {
        total: Some(total),
        processed: *processed,
      });
    }
  });
}

fn run_streets_sequential(
  conn: &rusqlite::Connection,
  tree: &RTree<spatial_entry>,
  entries: &[ancestor_entry],
  street_ids: &[i64],
  total: u64,
  processed: &mut u64,
  progress: &impl Fn(progress_report),
) {
  type row_t = crate::database::admin_levels_hierarchy::hierarchy_row;
  let mut batch: Vec<row_t> = Vec::new();

  for chunk in street_ids.chunks(READ_SIZE) {
    let rows = crate::database::admin_levels::load_by_ids(conn, chunk);
    for db_row in &rows {
      let e = parse_entry(db_row);
      let (ancestor_ids, user_friendly_name) = resolve_hierarchy(
        e.cx,
        e.cy,
        e.id,
        &e.name,
        e.own_post_code.as_deref(),
        e.admin_level,
        e.area,
        tree,
        entries,
      );
      batch.push(row_t {
        admin_level_id: e.id,
        ancestor_ids: ids_to_json(&ancestor_ids),
        user_friendly_name,
      });
      *processed += 1;
    }
    if batch.len() >= BATCH_SIZE {
      crate::database::admin_levels_hierarchy::batch_insert(conn, &batch);
      batch.clear();
      progress(progress_report {
        total: Some(total),
        processed: *processed,
      });
    }
  }

  if !batch.is_empty() {
    crate::database::admin_levels_hierarchy::batch_insert(conn, &batch);
    progress(progress_report {
      total: Some(total),
      processed: *processed,
    });
  }
}

fn parse_entry(row: &crate::database::admin_levels::admin_level_geom_row) -> ancestor_entry {
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
  let user_friendly_name = with_postcode(&row.name, own_post_code.as_deref());
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

fn with_postcode(base: &str, own_post_code: Option<&str>) -> String {
  match own_post_code.map(str::trim).filter(|s| !s.is_empty()) {
    Some(pc) => format!("{base}, {pc}"),
    None => base.to_string(),
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

#[allow(clippy::too_many_arguments)]
fn resolve_hierarchy(
  cx: f64,
  cy: f64,
  id: i64,
  name: &str,
  own_post_code: Option<&str>,
  current_level: u8,
  current_area: f64,
  tree: &RTree<spatial_entry>,
  entries: &[ancestor_entry],
) -> (Vec<i64>, String) {
  let mut by_level: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
  for se in tree.locate_in_envelope_intersecting(&AABB::from_point([cx, cy])) {
    let idx = se.idx;
    let c = &entries[idx];
    if c.id == id {
      continue;
    }
    if c.admin_level > current_level {
      continue;
    }
    if c.admin_level == current_level && c.area <= current_area {
      continue;
    }
    by_level.entry(c.admin_level).or_default().push(idx);
  }

  let mut parent: Option<usize> = None;
  for (_, level_candidates) in by_level.iter().rev() {
    for &idx in level_candidates {
      let c = &entries[idx];
      if !point_in_polygons(cx, cy, &c.polys) {
        continue;
      }
      if parent.is_none() || c.area < entries[parent.unwrap()].area {
        parent = Some(idx);
      }
    }
    if parent.is_some() {
      break;
    }
  }

  match parent {
    None => (vec![], with_postcode(name, own_post_code)),
    Some(idx) => {
      let p = &entries[idx];
      let mut ancestor_ids = vec![p.id];
      ancestor_ids.extend_from_slice(&p.ancestor_ids);
      let base = format!("{name}, {}", p.user_friendly_name);
      (ancestor_ids, with_postcode(&base, own_post_code))
    }
  }
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

// ray casting: O(n) per ring, returns true if (px, py) is inside
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

fn ids_to_json(ids: &[i64]) -> String {
  serde_json::to_string(ids).unwrap_or_else(|_| "[]".to_string())
}
