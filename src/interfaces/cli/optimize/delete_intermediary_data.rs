use std::time::Instant;

use rusqlite::Connection;

use crate::database::store;
use crate::interfaces::cli::exec::stages::{stage, stage_context};
use crate::osm_pbf_file::osm_pbf_file;

fn inside(data_path: &str, path: &str) -> bool {
  let canonical = |p: &str| std::fs::canonicalize(p).unwrap_or_else(|_| std::path::PathBuf::from(p));
  canonical(path).starts_with(canonical(data_path))
}

// the pbfs inside the data path and the osm_data every source shares, once the bases hold what
// the sources brought
pub fn command_handler_optimize_delete_intermediary_data(data_path: &str) -> bool {
  let start = Instant::now();
  let mut deleted = 0u32;
  {
    let conn = store::open_osm_data_write(data_path);
    let opened: Vec<Connection> = store::base_pool::new(data_path)
      .on_disk()
      .iter()
      .map(|base| store::open_base_readonly(&base.sqlite))
      .collect();
    stage::optimize_delete_intermediary_data.require(&stage_context {
      osm_data: Some(&conn),
      file: None,
      bases: opened.iter().collect(),
    });
    // a pbf the user keeps outside the data path is the user's: only one downloaded or copied
    // into the data path is deleted with its chunks
    let file = osm_pbf_file::open(Some(&conn), data_path);
    for path in crate::osm_pbf_file::repository::known_paths(&conn) {
      if !inside(data_path, &path) || !std::path::Path::new(&path).exists() {
        continue;
      }
      let bytes = file.delete(&path);
      deleted += 1;
      println!(
        "\x1b[1;32mdeleted\x1b[0m {}  {}",
        store::file_name_of(&path),
        super::fmt_size(bytes)
      );
    }
  }
  let bytes = store::remove_osm_data_files(data_path);
  deleted += 1;
  println!(
    "\x1b[1;32mdeleted\x1b[0m {}  {}",
    store::OSM_DATA_FILE,
    super::fmt_size(bytes)
  );
  let total = start.elapsed().as_secs_f64();
  println!("\x1b[1;32mdeleted\x1b[0m {deleted} tables in {total:.1}s");
  true
}
