use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::entity::house_number_link;
use super::linker::{self, street, tile_data};
use super::policy::house_number_policy;
use super::repository::{self, candidate_row, street_meta_row};

const TILE_SIZE: f64 = 2.0;
const WKB_BATCH: usize = 500;
const CHUNK_SIZE: usize = 500;

pub struct progress_report {
  pub total: u64,
  pub processed: u64,
}

type tile = (i64, i64);

impl house_number_link {
  pub fn extract(
    conn: &Connection,
    policy: &house_number_policy,
    mut on_progress: impl FnMut(progress_report),
  ) {
    let candidates = repository::load_all_candidates(conn, policy);
    let total = candidates.len() as u64;
    on_progress(progress_report {
      total,
      processed: 0,
    });
    if total == 0 {
      return;
    }

    let meta = repository::streets_with_centroid(conn);
    let by_tile = tiles_of(candidates);
    let streets = load_streets(conn, &meta, &by_tile);
    let links = link_in_parallel(tiles_with_streets(by_tile, &meta, &streets));

    let mut processed: u64 = 0;
    for chunk in links.chunks(CHUNK_SIZE) {
      processed += repository::batch_insert_links(conn, chunk) as u64;
      on_progress(progress_report { total, processed });
    }
  }
}

fn tile_of(lon: f64, lat: f64) -> tile {
  (
    (lon / TILE_SIZE).floor() as i64,
    (lat / TILE_SIZE).floor() as i64,
  )
}

fn covers((gx, gy): tile, cx: f64, cy: f64) -> bool {
  let lon_min = (gx - 1) as f64 * TILE_SIZE;
  let lon_max = (gx + 2) as f64 * TILE_SIZE;
  let lat_min = (gy - 1) as f64 * TILE_SIZE;
  let lat_max = (gy + 2) as f64 * TILE_SIZE;
  cx >= lon_min && cx < lon_max && cy >= lat_min && cy < lat_max
}

fn tiles_of(candidates: Vec<candidate_row>) -> HashMap<tile, Vec<candidate_row>> {
  let mut by_tile: HashMap<tile, Vec<candidate_row>> = HashMap::new();
  for c in candidates {
    by_tile.entry(tile_of(c.lon, c.lat)).or_default().push(c);
  }
  by_tile
}

fn load_streets(
  conn: &Connection,
  meta: &[street_meta_row],
  by_tile: &HashMap<tile, Vec<candidate_row>>,
) -> HashMap<i64, Arc<street>> {
  let mut needed: HashSet<i64> = HashSet::new();
  for &tile in by_tile.keys() {
    needed.extend(
      meta
        .iter()
        .filter(|m| covers(tile, m.cx, m.cy))
        .map(|m| m.id),
    );
  }
  let names: HashMap<i64, &str> = meta.iter().map(|m| (m.id, m.name.as_str())).collect();
  let ids: Vec<i64> = needed.into_iter().collect();
  let mut streets: HashMap<i64, Arc<street>> = HashMap::new();
  for chunk in ids.chunks(WKB_BATCH) {
    for row in repository::streets_wkb_by_ids(conn, chunk) {
      if let Some(street) =
        street::from_geometry(row.id, names[&row.id].to_string(), row.wkb.geometry())
      {
        streets.insert(row.id, Arc::new(street));
      }
    }
  }
  streets
}

fn tiles_with_streets(
  by_tile: HashMap<tile, Vec<candidate_row>>,
  meta: &[street_meta_row],
  streets: &HashMap<i64, Arc<street>>,
) -> Vec<tile_data> {
  by_tile
    .into_iter()
    .map(|(tile, candidates)| tile_data {
      streets: meta
        .iter()
        .filter(|m| covers(tile, m.cx, m.cy))
        .filter_map(|m| streets.get(&m.id).cloned())
        .collect(),
      candidates,
    })
    .collect()
}

fn link_in_parallel(tiles: Vec<tile_data>) -> Vec<house_number_link> {
  let threads = std::thread::available_parallelism()
    .map(std::num::NonZeroUsize::get)
    .unwrap_or(4);
  let mut groups: Vec<Vec<tile_data>> = (0..threads).map(|_| Vec::new()).collect();
  for (i, tile) in tiles.into_iter().enumerate() {
    groups[i % threads].push(tile);
  }
  let handles: Vec<_> = groups
    .into_iter()
    .map(|group| {
      std::thread::spawn(move || {
        group
          .into_iter()
          .flat_map(linker::link_tile)
          .collect::<Vec<_>>()
      })
    })
    .collect();
  handles
    .into_iter()
    .flat_map(|h| h.join().expect("worker thread panicked"))
    .collect()
}

#[cfg(test)]
#[path = "extract.test.rs"]
mod tests;
