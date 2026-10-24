use std::time::Instant;

use rusqlite::Connection;

use crate::database::store;
use crate::database::table;
use crate::house_number::{house_number_link, house_numbers};
use crate::interfaces::cli::exec::stages::{stage, stage_context};
use crate::interfaces::cli::progress;

pub fn command_handler_extract_osm_house_numbers(data_path: &str, recreate: bool) -> Vec<String> {
  let conn = store::open_osm_data(data_path);
  let pool = store::base_pool::new(data_path);
  let on_disk = pool.writable_on_disk();
  if recreate {
    for base in &on_disk {
      store::destroy_base(&base.sqlite, false, true);
    }
  }
  let opened: Vec<Connection> = on_disk
    .iter()
    .map(|base| store::open_base_write(&base.sqlite))
    .collect();
  let bases: Vec<&Connection> = opened.iter().collect();
  stage::extract_osm_house_numbers.require(&stage_context {
    osm_data: Some(&conn),
    file: None,
    bases: bases.clone(),
  });

  let bar = progress::bar("extracting", "house-numbers");

  let start = Instant::now();

  let count = house_number_link::extract(&conn, &bases, |p| progress::advance(&bar, &p));

  bar.finish();
  for base in &bases {
    house_numbers::create_indexes(base);
  }

  let elapsed = start.elapsed().as_secs_f64();
  println!("\x1b[1;32mextracted\x1b[0m {count} house numbers in {elapsed:.1}s");

  drop(opened);
  let paths: Vec<String> = on_disk.iter().map(|base| base.sqlite.clone()).collect();
  crate::osm_pbf_file::osm_pbf_file::open(Some(&conn), data_path).stamp_house_numbers_count(&paths);
  on_disk.into_iter().map(|base| base.iso).collect()
}
