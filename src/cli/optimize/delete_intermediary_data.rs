use std::time::Instant;

use crate::domain::osm_pbf_file::{listing, osm_pbf_file, source};

pub fn command_handler_optimize_delete_intermediary_data(data_path: &str, sqlite_path: &str) -> bool {
  crate::cli::require_sqlite(sqlite_path);
  let start = Instant::now();
  let mut deleted = 0u32;
  {
    let conn = crate::database::open_write(sqlite_path);
    if crate::domain::admin_level::repository::count_with_geometry(&conn) == 0 {
      eprintln!("\x1b[1;31merror\x1b[0m: admin_levels is empty — run extract first");
      return false;
    }
    if crate::domain::admin_level_hierarchy::repository::count(&conn) == 0 {
      eprintln!("\x1b[1;31merror\x1b[0m: admin_levels_hierarchy is empty — run index first");
      return false;
    }
    let file = osm_pbf_file::open(Some(&conn), data_path);
    let listing::local(files) = file.list(source::local, None, false) else {
      unreachable!("a local listing is always local");
    };
    for local in files {
      let path = local.path.to_string_lossy().into_owned();
      if !path.ends_with(".osm.pbf") {
        continue;
      }
      let name = local
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
      let bytes = file.delete(&path);
      deleted += 1;
      println!("\x1b[1;32mdeleted\x1b[0m {name}  {}", super::fmt_size(bytes));
    }
  }
  let bytes = crate::database::remove_osm_data_files(sqlite_path);
  deleted += 1;
  println!(
    "\x1b[1;32mdeleted\x1b[0m osm_data.sqlite3  {}",
    super::fmt_size(bytes)
  );
  let total = start.elapsed().as_secs_f64();
  println!("\x1b[1;32mdeleted\x1b[0m {deleted} tables in {total:.1}s");
  true
}
