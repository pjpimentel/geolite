use std::time::Instant;

use crate::house_number::{house_number_link, house_number_policy, house_numbers};
use crate::database::table;
use crate::interfaces::cli::progress;

pub fn command_handler_extract_osm_house_numbers(
  sqlite_path: &str,
  recreate: bool,
  policy: house_number_policy,
) {
  if recreate {
    crate::database::destroy_data(sqlite_path, false, false, false, true);
  }
  let conn = crate::database::open_write(sqlite_path);

  let bar = progress::bar("extracting", "house-numbers");

  let start = Instant::now();
  let mut count: u64 = 0;

  house_number_link::extract(&conn, &policy, |p| {
    progress::advance(&bar, &p);
    count = p.processed;
  });

  bar.finish();
  house_numbers::create_indexes(&conn);

  let elapsed = start.elapsed().as_secs_f64();
  println!("\x1b[1;32mextracted\x1b[0m {count} house numbers in {elapsed:.1}s");

  crate::osm_pbf_file::repository::update_house_numbers_count(&conn);
}
