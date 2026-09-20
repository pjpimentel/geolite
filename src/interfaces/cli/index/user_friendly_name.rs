use std::io::Write;
use std::path::Path;
use std::time::Instant;

use crate::interfaces::cli::progress;

pub fn command_handler_index_user_friendly_name(
  sqlite_path: &str,
  index_path: &str,
  preset: &crate::presets::index_user_friendly_name_preset,
) {
  crate::interfaces::cli::require_sqlite(sqlite_path);
  let conn = crate::database::open_write(sqlite_path);
  if crate::admin_level::repository::count_with_geometry(&conn) == 0 {
    eprintln!("\x1b[1;31merror\x1b[0m: admin_levels is empty — run extract first");
    return;
  }
  let path = Path::new(index_path);
  print!("\x1b[1;32mclearing\x1b[0m user-friendly-name...");
  let _ = std::io::stdout().flush();
  crate::admin_level_hierarchy::search_index::destroy(path);
  println!(" done");

  let bar = progress::bar("indexing", "user-friendly-name");

  let start = Instant::now();

  let _ = crate::admin_level_hierarchy::search_index::run(
    &conn,
    path,
    preset,
    |p| progress::advance(&bar, &p),
  );

  bar.finish();

  let elapsed = start.elapsed().as_secs_f64();
  println!("\x1b[1;32mindexed\x1b[0m user-friendly-name in {elapsed:.1}s");
}
