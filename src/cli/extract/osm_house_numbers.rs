use indicatif::{ProgressBar, ProgressStyle};
use std::time::Instant;

pub fn command_handler_extract_osm_house_numbers(
  sqlite_path: &str,
  recreate: bool,
  preset: crate::presets::extract_house_numbers_preset,
) {
  if recreate {
    crate::database::destroy_data(sqlite_path, false, false, false, true);
  }
  let conn = crate::database::open_write(sqlite_path);

  let bar = ProgressBar::new_spinner();
  bar.set_style(
    ProgressStyle::with_template(
      "{prefix:.bold.green} {msg:<40}  [{bar:20.green/white}] {percent:>3}%  {pos:>6}/{len:<6}  {per_sec}  eta {eta}",
    )
    .unwrap()
    .progress_chars("=> "),
  );
  bar.set_prefix("extracting");
  bar.set_message("house-numbers");

  let start = Instant::now();

  crate::extract::house_numbers::run(&conn, preset, |p| {
    if bar.length().is_none() && p.total > 0 {
      bar.set_length(p.total);
    }
    bar.set_position(p.processed);
  });

  bar.finish();
  crate::database::house_numbers::create_indexes(&conn);

  let count = bar.position();
  let elapsed = start.elapsed().as_secs_f64();
  println!("\x1b[1;32mextracted\x1b[0m {count} house numbers in {elapsed:.1}s");

  crate::database::osm_pbf_files::update_house_numbers_count(&conn);
}
