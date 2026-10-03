use std::time::Instant;

use crate::admin_level_hierarchy::isolated;
use crate::interfaces::cli::exec::stages::stage;

pub fn command_handler_optimize_delete_isolated_admin_levels(
  sqlite_path: &str,
  index_path: &str,
) -> bool {
  crate::interfaces::cli::require_sqlite(sqlite_path);
  let conn = crate::database::open_write(sqlite_path);
  stage::optimize_delete_isolated_admin_levels.require(&conn, None);
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
  super::update_ledger(&conn);

  let elapsed = start.elapsed().as_secs_f64();
  println!(
    "\x1b[1;32mdeleted\x1b[0m {} countries without children and {} streets without parents in {elapsed:.1}s, {} house numbers deleted",
    report.countries,
    report.streets,
    report.house_numbers
  );
  true
}
