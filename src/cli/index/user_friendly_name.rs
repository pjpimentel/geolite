use indicatif::ProgressStyle;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

pub fn command_handler_index_user_friendly_name(
  sqlite_path: &str,
  index_path: &str,
  preset: &crate::presets::index_user_friendly_name_preset,
) {
  crate::cli::require_sqlite(sqlite_path);
  let conn = crate::database::open_write(sqlite_path);
  if crate::database::admin_levels::count_with_geometry(&conn) == 0 {
    eprintln!("\x1b[1;31merror\x1b[0m: admin_levels is empty — run extract first");
    return;
  }
  let path = Path::new(index_path);
  print!("\x1b[1;32mclearing\x1b[0m user-friendly-name...");
  let _ = std::io::stdout().flush();
  crate::index::admin_levels_hierarchy_tantivy::destroy(path);
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
  bar.set_message("user-friendly-name");

  let start = Instant::now();

  let _ = crate::index::user_friendly_name::run(
    &conn,
    path,
    preset,
    |p| {
      if let Some(total) = p.total
        && bar.length().is_none()
      {
        bar.set_length(total as u64);
      }
      bar.set_position(p.processed as u64);
    },
  );

  bar.finish();

  let elapsed = start.elapsed().as_secs_f64();
  println!("\x1b[1;32mindexed\x1b[0m user-friendly-name in {elapsed:.1}s");
}
