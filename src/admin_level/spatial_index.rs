use geo::{BoundingRect, Closest, ClosestPoint, Geometry, HaversineDistance, LineString, Point};
use rusqlite::Connection;
use std::num::NonZeroUsize;
use std::sync::mpsc;
use std::thread;

use super::geometry::{admin_geometry, bounding_box};
use super::repository;
use super::scale::level;

const SQL_CREATE_RTREE: &str = "
  CREATE VIRTUAL TABLE IF NOT EXISTS admin_levels_rtree
  USING rtree(id, min_lon, max_lon, min_lat, max_lat);
";

const SQL_DROP_RTREE: &str = "DROP TABLE IF EXISTS admin_levels_rtree;";

pub struct rtree_row {
  pub id: i64,
  pub min_lon: f64,
  pub max_lon: f64,
  pub min_lat: f64,
  pub max_lat: f64,
}

pub(crate) fn create_table(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE_RTREE)
    .expect("failed to create admin_levels_rtree");
}

pub(crate) fn drop_table(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP_RTREE)
    .expect("failed to drop admin_levels_rtree");
}

pub fn recreate(conn: &Connection) {
  drop_table(conn);
  create_table(conn);
}

pub fn batch_insert(conn: &Connection, rows: &[rtree_row]) {
  const SQL_INSERT_RTREE: &str = "
    INSERT INTO admin_levels_rtree (
      id,
      min_lon,
      max_lon,
      min_lat,
      max_lat
    ) VALUES (
      ?1,
      ?2,
      ?3,
      ?4,
      ?5
    );
  ";

  if rows.is_empty() {
    return;
  }
  let tx = conn
    .unchecked_transaction()
    .expect("failed to begin transaction");
  {
    let mut stmt = tx
      .prepare(SQL_INSERT_RTREE)
      .expect("failed to prepare rtree insert");
    rows
      .iter()
      .try_for_each(|row| {
        stmt
          .execute(rusqlite::params![
            row.id,
            row.min_lon,
            row.max_lon,
            row.min_lat,
            row.max_lat
          ])
          .map(|_| ())
      })
      .expect("failed to insert rtree row");
  }
  tx.commit().expect("failed to commit rtree batch");
}

pub struct progress_report {
  pub total: Option<u64>,
  pub processed: u64,
}

const BATCH_SIZE: usize = 50_000;
const MAX_WORKERS: usize = 8;

pub fn run(conn: &Connection, progress: impl Fn(progress_report)) {
  recreate(conn);

  let total = repository::count_with_geometry(conn) as u64;
  progress(progress_report {
    total: Some(total),
    processed: 0,
  });

  if total == 0 {
    return;
  }

  let (min_id, max_id) = repository::id_range_with_geometry(conn);

  // conn.path() is Some("") for `:memory:`, and the workers would reopen that empty path, so an
  // in-memory database is scanned on this thread
  match conn.path().filter(|p| !p.is_empty()) {
    Some(path) => run_parallel(conn, path.to_owned(), min_id, max_id, total, progress),
    None => run_sequential(conn, min_id, max_id, total, progress),
  }
}

fn make_batch(page: Vec<(i64, admin_geometry)>) -> Vec<rtree_row> {
  page
    .into_iter()
    .filter_map(|(id, geom)| {
      geom.geometry().bounding_rect().map(|bbox| rtree_row {
        id,
        min_lon: bbox.min().x,
        max_lon: bbox.max().x,
        min_lat: bbox.min().y,
        max_lat: bbox.max().y,
      })
    })
    .collect()
}

fn scan_range(path: &str, range_start: i64, range_end: i64, tx: &mpsc::Sender<Vec<rtree_row>>) {
  let Ok(reader) = Connection::open_with_flags(
    path,
    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
  ) else {
    return;
  };

  let mut last_id = range_start - 1;
  loop {
    let page = repository::load_wkb_page(&reader, last_id, range_end, BATCH_SIZE);
    if page.is_empty() {
      break;
    }
    last_id = page.last().unwrap().0;
    tx.send(make_batch(page)).ok();
  }
}

fn run_parallel(
  conn: &Connection,
  path: String,
  min_id: i64,
  max_id: i64,
  total: u64,
  progress: impl Fn(progress_report),
) {
  let n_workers = thread::available_parallelism()
    .map(NonZeroUsize::get)
    .unwrap_or(4)
    .min(MAX_WORKERS);
  let range_size = (max_id - min_id) / n_workers as i64 + 1;

  let (tx, rx) = mpsc::channel::<Vec<rtree_row>>();

  thread::scope(|s| {
    for i in 0..n_workers {
      let tx = tx.clone();
      let path = path.clone();
      let range_start = min_id + i as i64 * range_size;
      let range_end = if i + 1 == n_workers {
        max_id
      } else {
        range_start + range_size - 1
      };
      s.spawn(move || scan_range(&path, range_start, range_end, &tx));
    }

    drop(tx);

    let mut processed = 0u64;
    for batch in rx {
      processed += batch.len() as u64;
      batch_insert(conn, &batch);
      progress(progress_report {
        total: Some(total),
        processed,
      });
    }
  });
}

fn run_sequential(
  conn: &Connection,
  min_id: i64,
  max_id: i64,
  total: u64,
  progress: impl Fn(progress_report),
) {
  let mut last_id = min_id - 1;
  let mut processed = 0u64;

  loop {
    let page = repository::load_wkb_page(conn, last_id, max_id, BATCH_SIZE);
    if page.is_empty() {
      break;
    }
    last_id = page.last().unwrap().0;
    let batch = make_batch(page);
    processed += batch.len() as u64;
    batch_insert(conn, &batch);
    progress(progress_report {
      total: Some(total),
      processed,
    });
  }
}

const RTREE_DELTA_DEG: f64 = 0.1;

pub struct nearest_street {
  pub id: i64,
  pub level: level,
  pub closest_point: Point<f64>,
  pub distance_in_meters: Option<u32>,
}

struct street_query_row {
  id: i64,
  level: level,
  wkb: Option<admin_geometry>,
}

fn streets_around(conn: &Connection, point: Point<f64>, envelope: bounding_box) -> Vec<street_query_row> {
  const SQL_STREETS_AROUND: &str = "
    SELECT
      al.id,
      al.admin_level,
      al.wkb
    FROM admin_levels al
    INNER JOIN admin_levels_rtree rt ON al.id = rt.id
    WHERE rt.min_lon <= ?1 AND rt.max_lon >= ?2
      AND rt.min_lat <= ?3 AND rt.max_lat >= ?4
      AND rt.min_lon <= ?5 AND rt.max_lon >= ?6
      AND rt.min_lat <= ?7 AND rt.max_lat >= ?8
      AND al.admin_level = ?9
  ";

  let (lon, lat) = (point.x(), point.y());
  let mut stmt = conn
    .prepare(SQL_STREETS_AROUND)
    .expect("failed to prepare streets around a point");
  stmt
    .query_map(
      rusqlite::params![
        lon + RTREE_DELTA_DEG,
        lon - RTREE_DELTA_DEG,
        lat + RTREE_DELTA_DEG,
        lat - RTREE_DELTA_DEG,
        envelope.max_lon,
        envelope.min_lon,
        envelope.max_lat,
        envelope.min_lat,
        level::street.value()
      ],
      |row| {
        let id: i64 = row.get(0)?;
        let wkb: Option<admin_geometry> = row.get(2).ok().flatten();
        Ok(level::new(row.get(1)?).map(|level| street_query_row { id, level, wkb }))
      },
    )
    .expect("failed to query streets around a point")
    .filter_map(|r| r.expect("failed to read street row"))
    .collect()
}

// the streets nearest to a point: the rtree narrows them to a window of 0.1° around the point
// inside the envelope, each one's closest point is measured on the ground, and the answer comes
// most specific level first, closest first within a level
pub fn nearest(conn: &Connection, point: Point<f64>, envelope: bounding_box) -> Vec<nearest_street> {
  let raw = streets_around(conn, point, envelope);
  crate::debug!("debug: rtree raw={} for lon={} lat={}", raw.len(), point.x(), point.y());

  let mut rej_wkt_none = 0;
  let mut rej_not_linestring = 0;
  let mut rej_empty = 0;
  let mut rej_indeterminate = 0;
  let mut min_dist = f64::MAX;

  let mut candidates: Vec<nearest_street> = Vec::new();
  for s in raw {
    let geom = match s.wkb {
      Some(g) => g.into_geometry(),
      None => {
        rej_wkt_none += 1;
        continue;
      }
    };
    let linestrings: Vec<LineString<f64>> = match geom {
      Geometry::LineString(ls) => vec![ls],
      Geometry::MultiLineString(mls) => mls.0,
      _ => {
        rej_not_linestring += 1;
        continue;
      }
    };
    let mut best: Option<(Point<f64>, f64)> = None;
    let mut had_non_empty = false;
    for ls in &linestrings {
      if ls.0.is_empty() {
        continue;
      }
      had_non_empty = true;
      if let Closest::SinglePoint(p) | Closest::Intersection(p) = ls.closest_point(&point) {
        let d = point.haversine_distance(&p);
        if best.is_none_or(|(_, bd)| d < bd) {
          best = Some((p, d));
        }
      }
    }
    if !had_non_empty {
      rej_empty += 1;
      continue;
    }
    let (closest_point, dist) = match best {
      Some(b) => b,
      None => {
        rej_indeterminate += 1;
        continue;
      }
    };
    if dist < min_dist {
      min_dist = dist;
    }
    candidates.push(nearest_street {
      id: s.id,
      level: s.level,
      closest_point,
      distance_in_meters: Some(dist.round().clamp(0.0, u32::MAX as f64) as u32),
    });
  }

  crate::debug!(
    "debug: rejections wkt_none={} not_linestring={} empty={} indeterminate={} min_dist_m={:.2}",
    rej_wkt_none,
    rej_not_linestring,
    rej_empty,
    rej_indeterminate,
    min_dist
  );

  candidates.sort_by(|a, b| {
    b.level
      .cmp(&a.level)
      .then(a.distance_in_meters.cmp(&b.distance_in_meters))
  });
  candidates
}

pub fn ids_in_bounding_box(conn: &Connection, bbox: bounding_box) -> Vec<i64> {
  const SQL_IDS_IN_BOUNDING_BOX: &str = "
    SELECT id
    FROM admin_levels_rtree
    WHERE min_lon <= ?1 AND max_lon >= ?2
      AND min_lat <= ?3 AND max_lat >= ?4
  ";

  let mut stmt = conn
    .prepare(SQL_IDS_IN_BOUNDING_BOX)
    .expect("failed to prepare ids_in_bounding_box");
  stmt
    .query_map(
      rusqlite::params![bbox.max_lon, bbox.min_lon, bbox.max_lat, bbox.min_lat],
      |row| row.get::<_, i64>(0),
    )
    .expect("failed to query ids_in_bounding_box")
    .map(|r| r.expect("failed to read bounding box id"))
    .collect()
}
