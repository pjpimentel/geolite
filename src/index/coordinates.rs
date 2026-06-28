use geo::BoundingRect;
use std::sync::mpsc;
use std::thread;

pub struct progress_report {
  pub total: Option<u64>,
  pub processed: u64,
}

const BATCH_SIZE: usize = 50_000;
const MAX_WORKERS: usize = 8;

pub fn run(conn: &rusqlite::Connection, progress: impl Fn(progress_report)) {
  crate::database::admin_levels::recreate_rtree(conn);

  let total = crate::database::admin_levels::count_with_geometry(conn) as u64;
  progress(progress_report {
    total: Some(total),
    processed: 0,
  });

  if total == 0 {
    return;
  }

  let (min_id, max_id) = crate::database::admin_levels::id_range_with_geometry(conn);

  // conn.path() retorna Some("") para `:memory:` — nesse caso a versao parallel falha
  // porque os workers tentam reabrir um path vazio. tratamos como sequential.
  match conn.path().filter(|p| !p.is_empty()) {
    Some(path) => run_parallel(conn, path.to_owned(), min_id, max_id, total, progress),
    None => run_sequential(conn, min_id, max_id, total, progress),
  }
}

fn make_batch(
  page: Vec<(i64, crate::database::admin_levels::admin_geometry)>,
) -> Vec<crate::database::admin_levels::rtree_row> {
  page
    .into_iter()
    .filter_map(|(id, geom)| {
      geom
        .geometry()
        .bounding_rect()
        .map(|bbox| crate::database::admin_levels::rtree_row {
          id,
          min_lon: bbox.min().x,
          max_lon: bbox.max().x,
          min_lat: bbox.min().y,
          max_lat: bbox.max().y,
        })
    })
    .collect()
}

fn run_parallel(
  conn: &rusqlite::Connection,
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

  let (tx, rx) = mpsc::channel::<Vec<crate::database::admin_levels::rtree_row>>();

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

      s.spawn(move || {
        let Ok(reader) = rusqlite::Connection::open_with_flags(
          &path,
          rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) else {
          return;
        };

        let mut last_id = range_start - 1;
        loop {
          let page =
            crate::database::admin_levels::load_wkb_page(&reader, last_id, range_end, BATCH_SIZE);
          if page.is_empty() {
            break;
          }
          last_id = page.last().unwrap().0;
          let batch = make_batch(page);
          tx.send(batch).ok();
        }
      });
    }

    drop(tx);

    let mut processed = 0u64;
    for batch in rx {
      processed += batch.len() as u64;
      crate::database::admin_levels::batch_insert_rtree(conn, &batch);
      progress(progress_report {
        total: Some(total),
        processed,
      });
    }
  });
}

fn run_sequential(
  conn: &rusqlite::Connection,
  min_id: i64,
  max_id: i64,
  total: u64,
  progress: impl Fn(progress_report),
) {
  let mut last_id = min_id - 1;
  let mut processed = 0u64;

  loop {
    let page = crate::database::admin_levels::load_wkb_page(conn, last_id, max_id, BATCH_SIZE);
    if page.is_empty() {
      break;
    }
    last_id = page.last().unwrap().0;
    let batch = make_batch(page);
    processed += batch.len() as u64;
    crate::database::admin_levels::batch_insert_rtree(conn, &batch);
    progress(progress_report {
      total: Some(total),
      processed,
    });
  }
}
