use rusqlite::Connection;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::entity::house_number_link;
use super::linker::{self, street, tile_data};
use super::repository::{self, candidate_row};
use crate::admin_level::repository::street_centre_row;
use crate::progress_report;

const TILE_SIZE: f64 = 2.0;
const WKB_BATCH: usize = 500;
const CHUNK_SIZE: usize = 500;

type tile = (i64, i64);

// the streets of every base a source may link to, each street remembering its base
struct street_meta {
  rows: Vec<street_centre_row>,
  base_of: HashMap<i64, usize>,
}

impl house_number_link {
  // the candidates come from the osm_data of the source, the streets from every base of the data
  // path, and every link lands in the base of its street
  pub fn extract(
    conn: &Connection,
    bases: &[&Connection],
    mut on_progress: impl FnMut(progress_report),
  ) -> u64 {
    let candidates = repository::load_all_candidates(conn);
    let total = candidates.len() as u64;
    on_progress(progress_report {
      total: Some(total),
      processed: 0,
    });
    if total == 0 {
      return 0;
    }

    let meta = load_meta(bases);
    let by_tile = tiles_of(candidates);
    let streets = load_streets(bases, &meta, &by_tile);
    let mut links = link_in_parallel(tiles_with_streets(by_tile, &meta.rows, &streets));
    drop_non_values(&mut links, &meta.rows);
    links.sort_unstable_by_key(|link| link.node_id);

    let mut by_base: Vec<Vec<house_number_link>> = (0..bases.len()).map(|_| Vec::new()).collect();
    for link in links {
      let base = meta.base_of[&(link.street_id.raw() as i64)];
      by_base[base].push(link);
    }
    let mut processed: u64 = 0;
    for (base, links) in bases.iter().zip(by_base) {
      for chunk in links.chunks(CHUNK_SIZE) {
        processed += repository::batch_insert_links(base, chunk) as u64;
        on_progress(progress_report {
          total: Some(total),
          processed,
        });
      }
    }
    processed
  }
}

fn load_meta(bases: &[&Connection]) -> street_meta {
  let mut rows: Vec<street_centre_row> = Vec::new();
  let mut base_of: HashMap<i64, usize> = HashMap::new();
  for (at, base) in bases.iter().enumerate() {
    for row in crate::admin_level::repository::streets_with_centroid(base) {
      base_of.insert(row.id, at);
      rows.push(row);
    }
  }
  street_meta { rows, base_of }
}

// a value that means "no number" is the country's to name (`s/n` in portuguese and spanish), and
// the country of a node is the one of the street it was linked to
fn drop_non_values(links: &mut Vec<house_number_link>, meta: &[street_centre_row]) {
  let country_of: HashMap<i64, Option<&str>> = meta
    .iter()
    .map(|m| (m.id, m.country_iso_code.as_deref()))
    .collect();
  links.retain(|link| {
    let iso = country_of
      .get(&(link.street_id.raw() as i64))
      .copied()
      .flatten();
    let policy = crate::presets::for_country(iso).house_numbers;
    !link.number.is_dropped(policy.drop_values)
  });
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
  bases: &[&Connection],
  meta: &street_meta,
  by_tile: &HashMap<tile, Vec<candidate_row>>,
) -> HashMap<i64, Arc<street>> {
  let mut needed: HashSet<i64> = HashSet::new();
  for &tile in by_tile.keys() {
    needed.extend(
      meta
        .rows
        .iter()
        .filter(|m| covers(tile, m.cx, m.cy))
        .map(|m| m.id),
    );
  }
  let names: HashMap<i64, &str> = meta.rows.iter().map(|m| (m.id, m.name.as_str())).collect();
  let mut by_base: Vec<Vec<i64>> = (0..bases.len()).map(|_| Vec::new()).collect();
  for id in needed {
    by_base[meta.base_of[&id]].push(id);
  }
  let mut streets: HashMap<i64, Arc<street>> = HashMap::new();
  for (base, ids) in bases.iter().zip(by_base) {
    for chunk in ids.chunks(WKB_BATCH) {
      for (id, wkb) in crate::admin_level::repository::geometry_by_ids(base, chunk) {
        if let Some(street) = street::from_geometry(id, names[&id].to_string(), wkb.geometry()) {
          streets.insert(id, Arc::new(street));
        }
      }
    }
  }
  streets
}

fn tiles_with_streets(
  by_tile: HashMap<tile, Vec<candidate_row>>,
  meta: &[street_centre_row],
  streets: &HashMap<i64, Arc<street>>,
) -> Vec<tile_data> {
  let mut tiles: Vec<(tile, Vec<candidate_row>)> = by_tile.into_iter().collect();
  tiles.sort_unstable_by_key(|(tile, _)| *tile);
  tiles
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
