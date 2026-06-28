use geo::{
  BoundingRect, Closest, ClosestPoint, EuclideanDistance, Geometry, LineString, MultiLineString,
  Point,
};
use rstar::{AABB, PointDistance, RTree, RTreeObject};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

const TILE_SIZE: f64 = 2.0;
// approximates the legacy 3x3 grid filter at 0.05° per cell —
// candidates further than this from the nearest street are skipped
const MAX_MATCH_DEG: f64 = 0.15;
const WKB_BATCH: usize = 500;
const CHUNK_SIZE: usize = 500;

const STRATEGY_BY_PROXIMITY: u8 = 0;
const STRATEGY_BY_NAME: u8 = 1;

struct street {
  // FK to admin_levels.id (internal autoincrement) — stays i64
  id: i64,
  name: String,
  geometry: MultiLineString<f64>,
  envelope: AABB<[f64; 2]>,
}

struct indexed_street {
  data: Arc<street>,
}

impl RTreeObject for indexed_street {
  type Envelope = AABB<[f64; 2]>;
  fn envelope(&self) -> Self::Envelope {
    self.data.envelope
  }
}

impl PointDistance for indexed_street {
  fn distance_2(&self, point: &[f64; 2]) -> f64 {
    let p = Point::new(point[0], point[1]);
    let d = p.euclidean_distance(&self.data.geometry);
    d * d
  }
}

pub struct progress_report {
  pub total: u64,
  pub processed: u64,
}

struct tile_data {
  streets: Vec<Arc<street>>,
  candidates: Vec<crate::database::house_numbers::candidate_row>,
}

fn geometry_to_multilinestring(geom: &Geometry<f64>) -> Option<MultiLineString<f64>> {
  let lines: Vec<LineString<f64>> = match geom {
    Geometry::MultiLineString(mls) => mls.0.clone(),
    Geometry::LineString(ls) => vec![ls.clone()],
    Geometry::Polygon(p) => {
      let mut v = vec![p.exterior().clone()];
      v.extend(p.interiors().iter().cloned());
      v
    }
    Geometry::MultiPolygon(mp) => {
      let mut v = Vec::new();
      for p in &mp.0 {
        v.push(p.exterior().clone());
        v.extend(p.interiors().iter().cloned());
      }
      v
    }
    _ => return None,
  };
  if lines.iter().all(|ls| ls.0.is_empty()) {
    return None;
  }
  Some(MultiLineString(lines))
}

fn closest_point_on_geometry(geom: &MultiLineString<f64>, p: &Point<f64>) -> Option<Point<f64>> {
  let mut best: Option<(Point<f64>, f64)> = None;
  for ls in &geom.0 {
    if ls.0.is_empty() {
      continue;
    }
    if let Closest::SinglePoint(cp) | Closest::Intersection(cp) = ls.closest_point(p) {
      let d = p.euclidean_distance(&cp);
      if best.is_none_or(|(_, bd)| d < bd) {
        best = Some((cp, d));
      }
    }
  }
  best.map(|(cp, _)| cp)
}

fn process_tile(tile: tile_data) -> Vec<crate::database::house_numbers::house_numbers> {
  let tile_data {
    streets,
    candidates,
  } = tile;

  let mut by_name: HashMap<String, Vec<Arc<street>>> = HashMap::new();
  for s in &streets {
    by_name
      .entry(s.name.to_lowercase())
      .or_default()
      .push(s.clone());
  }

  let indexed: Vec<indexed_street> = streets
    .iter()
    .map(|s| indexed_street { data: s.clone() })
    .collect();
  let tree = RTree::bulk_load(indexed);

  let max_dist_sq = MAX_MATCH_DEG * MAX_MATCH_DEG;

  let mut results: Vec<crate::database::house_numbers::house_numbers> = Vec::new();
  for c in candidates {
    let pt = Point::new(c.lon, c.lat);
    let mut best: Option<Arc<street>> = None;
    let mut strategy = STRATEGY_BY_PROXIMITY;

    if let Some(addr_street) = &c.addr_street
      && let Some(matches) = by_name.get(&addr_street.to_lowercase())
    {
      let mut min_d = f64::INFINITY;
      for s in matches {
        let d = pt.euclidean_distance(&s.geometry);
        if d < min_d {
          min_d = d;
          best = Some(s.clone());
        }
      }
      if best.is_some() {
        strategy = STRATEGY_BY_NAME;
      }
    }

    if best.is_none()
      && let Some(nearest) = tree.nearest_neighbor(&[c.lon, c.lat])
      && nearest.distance_2(&[c.lon, c.lat]) <= max_dist_sq
    {
      best = Some(nearest.data.clone());
    }

    if let Some(s) = best
      && let Some(cp) = closest_point_on_geometry(&s.geometry, &pt)
    {
      results.push(crate::database::house_numbers::house_numbers {
        node_id: c.id,
        admin_level_id: s.id,
        number: c.number,
        wkb: Geometry::Point(cp).into(),
        strategy,
      });
    }
  }
  results
}

pub fn run(
  conn: &rusqlite::Connection,
  preset: crate::presets::extract_house_numbers_preset,
  progress: impl Fn(progress_report),
) {
  let all_candidates = crate::database::house_numbers::load_all_candidates(
    conn,
    preset.housenumber_tags,
    preset.street_tags,
    preset.drop_values,
  );
  let total = all_candidates.len() as u64;

  progress(progress_report {
    total,
    processed: 0,
  });
  if total == 0 {
    return;
  }

  let street_meta_rows = crate::database::house_numbers::streets_with_centroid(conn);
  let mut meta_map: HashMap<i64, (String, f64, f64)> = HashMap::new();
  for m in &street_meta_rows {
    meta_map.insert(m.id, (m.name.clone(), m.cx, m.cy));
  }

  let mut by_tile: HashMap<(i64, i64), Vec<crate::database::house_numbers::candidate_row>> =
    HashMap::new();
  for c in all_candidates {
    let gx = (c.lon / TILE_SIZE).floor() as i64;
    let gy = (c.lat / TILE_SIZE).floor() as i64;
    by_tile.entry((gx, gy)).or_default().push(c);
  }

  // Pre-collect unique street IDs needed across all tiles (with ±1 tile expansion)
  let mut all_needed_ids: HashSet<i64> = HashSet::new();
  for &(gx, gy) in by_tile.keys() {
    let lon_min_ext = (gx - 1) as f64 * TILE_SIZE;
    let lon_max_ext = (gx + 2) as f64 * TILE_SIZE;
    let lat_min_ext = (gy - 1) as f64 * TILE_SIZE;
    let lat_max_ext = (gy + 2) as f64 * TILE_SIZE;
    for (&id, (_, cx, cy)) in &meta_map {
      if *cx >= lon_min_ext && *cx < lon_max_ext && *cy >= lat_min_ext && *cy < lat_max_ext {
        all_needed_ids.insert(id);
      }
    }
  }

  // Load all needed WKBs upfront in one pass — enables parallel tile processing
  let all_ids: Vec<i64> = all_needed_ids.into_iter().collect();
  let mut street_map: HashMap<i64, Arc<street>> = HashMap::new();
  for chunk in all_ids.chunks(WKB_BATCH) {
    let wkb_rows = crate::database::house_numbers::streets_wkb_by_ids(conn, chunk);
    for r in wkb_rows {
      let Some(mls) = geometry_to_multilinestring(r.wkb.geometry()) else {
        continue;
      };
      let Some(bbox) = mls.bounding_rect() else {
        continue;
      };
      let envelope =
        AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);
      let name = meta_map[&r.id].0.clone();
      street_map.insert(
        r.id,
        Arc::new(street {
          id: r.id,
          name,
          geometry: mls,
          envelope,
        }),
      );
    }
  }

  // Build per-tile data with Arc references into shared street segments
  let mut tiles: Vec<tile_data> = by_tile
    .into_iter()
    .map(|((gx, gy), candidates)| {
      let lon_min_ext = (gx - 1) as f64 * TILE_SIZE;
      let lon_max_ext = (gx + 2) as f64 * TILE_SIZE;
      let lat_min_ext = (gy - 1) as f64 * TILE_SIZE;
      let lat_max_ext = (gy + 2) as f64 * TILE_SIZE;
      let streets: Vec<Arc<street>> = meta_map
        .iter()
        .filter(|(_, (_, cx, cy))| {
          *cx >= lon_min_ext && *cx < lon_max_ext && *cy >= lat_min_ext && *cy < lat_max_ext
        })
        .filter_map(|(&id, _)| street_map.get(&id).cloned())
        .collect();
      tile_data {
        candidates,
        streets,
      }
    })
    .collect();

  // Distribute tiles across worker threads — buckets are independent
  let num_threads = std::thread::available_parallelism()
    .map(|n| n.get())
    .unwrap_or(4);
  let mut groups: Vec<Vec<tile_data>> = (0..num_threads).map(|_| Vec::new()).collect();
  for (i, tile) in tiles.drain(..).enumerate() {
    groups[i % num_threads].push(tile);
  }

  let handles: Vec<_> = groups
    .into_iter()
    .map(|group| {
      std::thread::spawn(move || group.into_iter().flat_map(process_tile).collect::<Vec<_>>())
    })
    .collect();

  let all_rows: Vec<crate::database::house_numbers::house_numbers> = handles
    .into_iter()
    .flat_map(|h| h.join().expect("worker thread panicked"))
    .collect();

  let mut processed: u64 = 0;
  for chunk in all_rows.chunks(CHUNK_SIZE) {
    processed += crate::database::house_numbers::batch_insert(conn, chunk) as u64;
    progress(progress_report { total, processed });
  }
}
