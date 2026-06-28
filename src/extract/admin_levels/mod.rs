pub mod level_10;
pub mod level_12;

use geo::{Coord, Geometry, LineString, MultiLineString, MultiPolygon, Polygon, Winding};
use rusqlite::Connection;
use std::sync::{Arc, Mutex, mpsc};

#[derive(Clone, Copy)]
pub enum osm_admin_level {
  continent = 1,
  country = 2,
  region = 3,
  state = 4,
  district = 5,
  county = 6,
  municipality = 7,
  city = 8,
  locality = 9,
  neighborhood = 10,
  street = 12,
  address = 14,
  house_numbers = 30,
}

impl TryFrom<u8> for osm_admin_level {
  type Error = u8;
  fn try_from(v: u8) -> Result<Self, Self::Error> {
    match v {
      1 => Ok(osm_admin_level::continent),
      2 => Ok(osm_admin_level::country),
      3 => Ok(osm_admin_level::region),
      4 => Ok(osm_admin_level::state),
      5 => Ok(osm_admin_level::district),
      6 => Ok(osm_admin_level::county),
      7 => Ok(osm_admin_level::municipality),
      8 => Ok(osm_admin_level::city),
      9 => Ok(osm_admin_level::locality),
      10 => Ok(osm_admin_level::neighborhood),
      12 => Ok(osm_admin_level::street),
      14 => Ok(osm_admin_level::address),
      30 => Ok(osm_admin_level::house_numbers),
      _ => Err(v),
    }
  }
}

pub struct progress_report {
  pub total: Option<u64>,
  pub processed: u64,
}

#[derive(Clone, Copy)]
pub struct extraction_rules {
  pub level: u8,
  pub include: &'static [crate::database::osm_ways::filters],
  pub exclude: &'static [crate::database::osm_ways::filters],
}

fn default_include(level: u8) -> &'static [crate::database::osm_ways::filters] {
  use crate::database::osm_ways::filters;
  match level {
    10 => &[
      filters::include_place_neighbourhood,
      filters::include_place_suburb,
    ],
    _ => &[],
  }
}

fn default_exclude(level: u8) -> &'static [crate::database::osm_ways::filters] {
  use crate::database::osm_ways::filters;
  match level {
    12 => &[
      filters::exclude_place_neighbourhood,
      filters::exclude_place_suburb,
      filters::exclude_leisure_park,
      filters::exclude_building,
      filters::exclude_waterway,
    ],
    _ => &[],
  }
}

pub fn resolve_rules(
  level: u8,
  overrides: &[extraction_rules],
) -> (
  &'static [crate::database::osm_ways::filters],
  &'static [crate::database::osm_ways::filters],
) {
  if let Some(r) = overrides.iter().find(|r| r.level == level) {
    return (r.include, r.exclude);
  }
  (default_include(level), default_exclude(level))
}

pub(super) const CHUNK_SIZE: usize = 500;

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

pub fn run_with_ids(
  conn: &Connection,
  ids: Vec<u64>,
  admin_level: osm_admin_level,
  threads: usize,
  name_priority: &[&str],
  progress: impl Fn(progress_report),
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
  let (result_tx, result_rx) =
    mpsc::channel::<Option<crate::database::admin_levels::admin_levels>>();

  std::thread::scope(|s| {
    for _ in 0..threads {
      let rx = work_rx.clone();
      let tx = result_tx.clone();
      s.spawn(move || {
        loop {
          let item = { rx.lock().unwrap().recv() };
          match item {
            Ok(w) => {
              tx.send(process_one_relation(
                w.relation_id,
                &w.meta,
                &w.ways,
                admin_level,
              ))
              .ok();
            }
            Err(_) => break,
          }
        }
      });
    }
    drop(result_tx);

    let mut processed: u64 = 0;

    for chunk in ids.chunks(CHUNK_SIZE) {
      let dispatched = load_and_send(conn, chunk, name_priority, &work_tx);

      let mut batch: Vec<crate::database::admin_levels::admin_levels> = Vec::new();
      for _ in 0..dispatched {
        if let Ok(Some(row)) = result_rx.recv() {
          batch.push(row);
        }
      }

      processed += crate::database::admin_levels::batch_upsert(conn, &batch) as u64;
      progress(progress_report {
        total: Some(total),
        processed,
      });
    }

    drop(work_tx);
  });
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
  admin_level: osm_admin_level,
) -> Option<crate::database::admin_levels::admin_levels> {
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

  Some(crate::database::admin_levels::admin_levels {
    relation_id: Some(relation_id),
    way_id: None,
    admin_level: admin_level as u8,
    name: meta.name.clone(),
    country_iso_code: meta.country_iso_code.clone(),
    post_code: meta.post_code.clone(),
    wkb: geom.into(),
  })
}

pub(super) fn approx_eq(a: Coord<f64>, b: Coord<f64>) -> bool {
  (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

fn assemble_rings(ways: &[LineString<f64>]) -> Vec<LineString<f64>> {
  let mut remaining: Vec<(LineString<f64>, bool)> =
    ways.iter().map(|w| (w.clone(), false)).collect();
  let mut rings: Vec<LineString<f64>> = Vec::new();
  while let Some(start_idx) = remaining.iter().position(|(_, used)| !used) {
    let mut coords: Vec<Coord<f64>> = remaining[start_idx].0.0.clone();
    remaining[start_idx].1 = true;
    let mut extended = true;
    while extended {
      extended = false;
      let tail = *coords.last().unwrap();
      for (ls, used) in remaining.iter_mut() {
        if *used {
          continue;
        }
        let pts = &ls.0;
        if approx_eq(pts[0], tail) {
          coords.extend_from_slice(&pts[1..]);
          *used = true;
          extended = true;
          break;
        }
        if approx_eq(pts[pts.len() - 1], tail) {
          let mut rev = pts.clone();
          rev.reverse();
          coords.extend_from_slice(&rev[1..]);
          *used = true;
          extended = true;
          break;
        }
      }
    }
    rings.push(LineString(coords));
  }
  rings
}
