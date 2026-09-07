use geo::BoundingRect;
use rusqlite::Connection;
use std::sync::mpsc;
use std::thread;

use super::geometry::admin_geometry;
use super::repository;

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
    .map(|n| n.get())
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

#[cfg(test)]
#[path = "spatial_index.test.rs"]
mod tests;
