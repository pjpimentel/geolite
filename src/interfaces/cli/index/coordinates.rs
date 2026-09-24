use std::io::Write;
use std::time::Instant;

use crate::interfaces::cli::exec::stages::stage;
use crate::interfaces::cli::progress;

pub fn command_handler_index_coordinates(sqlite_path: &str) {
  crate::interfaces::cli::require_sqlite(sqlite_path);
  let conn = crate::database::open_write(sqlite_path);
  stage::index_coordinates.require(&conn, None);
  print!("\x1b[1;32mclearing\x1b[0m coordinates...");
  let _ = std::io::stdout().flush();
  crate::admin_level::spatial_index::recreate(&conn);
  println!(" done");

  let bar = progress::bar("indexing", "coordinates");

  let start = Instant::now();

  crate::admin_level::spatial_index::run(&conn, |p| progress::advance(&bar, &p));

  bar.finish();

  let elapsed = start.elapsed().as_secs_f64();
  println!("\x1b[1;32mindexed\x1b[0m coordinates in {elapsed:.1}s");
}
