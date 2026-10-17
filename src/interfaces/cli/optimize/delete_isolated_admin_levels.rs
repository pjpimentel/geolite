use std::time::Instant;

use crate::admin_level_hierarchy::isolated;
use crate::database::store::{self, base_files};
use crate::interfaces::cli::exec::stages::{stage, stage_context};

pub fn command_handler_optimize_delete_isolated_admin_levels(
  sqlite_path: &str,
  index_path: &str,
) -> bool {
  crate::interfaces::cli::require_sqlite(sqlite_path);
  let conn = store::open_base_write(sqlite_path);
  stage::optimize_delete_isolated_admin_levels.require(&stage_context {
    osm_data: None,
    file: None,
    bases: vec![&conn],
  });
  let start = Instant::now();

  let found = isolated::find(&conn);
  if found.is_empty() {
    println!(
      "\x1b[1;32mskipping\x1b[0m delete-isolated-admin-levels — no country without children and no street without parents"
    );
    return false;
  }

  super::clear_indexes(&conn, index_path);
  let report = isolated::remove(&conn, &found);

  let elapsed = start.elapsed().as_secs_f64();
  println!(
    "\x1b[1;32mdeleted\x1b[0m {} countries without children and {} streets without parents in {elapsed:.1}s, {} house numbers deleted",
    report.countries,
    report.streets,
    report.house_numbers
  );
  if crate::admin_level::repository::count(&conn) == 0 {
    drop(conn);
    let base = base_files::of(sqlite_path);
    store::remove_base(&base);
    println!(
      "\x1b[1;33mdeleted\x1b[0m {} — no area left in it",
      base.label()
    );
    return false;
  }
  true
}
