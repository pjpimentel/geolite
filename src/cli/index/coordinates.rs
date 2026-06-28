use indicatif::ProgressStyle;
use std::io::Write;
use std::time::Instant;

pub fn command_handler_index_coordinates(sqlite_path: &str) {
  crate::cli::require_sqlite(sqlite_path);
  let conn = crate::database::open_write(sqlite_path);
  if crate::database::admin_levels::count_with_geometry(&conn) == 0 {
    eprintln!("\x1b[1;31merror\x1b[0m: admin_levels is empty — run extract first");
    return;
  }
  print!("\x1b[1;32mclearing\x1b[0m coordinates...");
  let _ = std::io::stdout().flush();
  crate::database::admin_levels::recreate_rtree(&conn);
  println!(" done");

  let bar = super::progress_bar();
  bar.set_style(
    ProgressStyle::with_template(
      "{prefix:.bold.green} {msg:<40}  [{bar:20.green/white}] {percent:>3}%  {pos:>6}/{len:<6}  {per_sec}  eta {eta}",
    )
    .unwrap()
    .progress_chars("=> "),
  );
  bar.set_prefix("indexing");
  bar.set_message("coordinates");

  let start = Instant::now();

  crate::index::coordinates::run(&conn, |p| {
    if let Some(total) = p.total
      && bar.length().is_none()
    {
      bar.set_length(total);
    }
    bar.set_position(p.processed);
  });

  bar.finish();

  let elapsed = start.elapsed().as_secs_f64();
  println!("\x1b[1;32mindexed\x1b[0m coordinates in {elapsed:.1}s");
}
