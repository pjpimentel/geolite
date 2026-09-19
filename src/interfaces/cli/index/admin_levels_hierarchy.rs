use std::io::Write;
use std::time::Instant;

use crate::interfaces::cli::progress;

pub fn command_handler_index_admin_levels_hierarchy(sqlite_path: &str) {
  crate::interfaces::cli::require_sqlite(sqlite_path);
  let conn = crate::database::open_write(sqlite_path);
  if crate::admin_level::repository::count_with_geometry(&conn) == 0 {
    eprintln!("\x1b[1;31merror\x1b[0m: admin_levels is empty — run extract first");
    return;
  }
  print!("\x1b[1;32mclearing\x1b[0m hierarchy...");
  let _ = std::io::stdout().flush();
  crate::admin_level_hierarchy::repository::destroy(&conn);
  println!(" done");

  let bar = progress::bar("indexing", "hierarchy");

  let start = Instant::now();

  crate::admin_level_hierarchy::resolver::run(&conn, |p| progress::advance(&bar, &p));

  bar.finish();

  let elapsed = start.elapsed().as_secs_f64();
  println!("\x1b[1;32mindexed\x1b[0m hierarchy in {elapsed:.1}s");
}
