use std::io::Write;
use std::time::Instant;

pub fn command_handler_optimize_sqlite_file(sqlite_path: &str) {
  crate::cli::require_sqlite(sqlite_path);
  print!("\x1b[1;32moptimizing\x1b[0m sqlite file...");
  let _ = std::io::stdout().flush();
  let start = Instant::now();
  let conn = crate::database::open_write_main(sqlite_path);
  let (bytes_before, bytes_after) = crate::optimize::sqlite_file::run(&conn);
  drop(conn);
  let elapsed = start.elapsed().as_secs_f64();
  println!(
    "\n\x1b[1;32moptimized\x1b[0m sqlite in {elapsed:.1}s  {} → {}",
    super::fmt_size(bytes_before),
    super::fmt_size(bytes_after),
  );
}
