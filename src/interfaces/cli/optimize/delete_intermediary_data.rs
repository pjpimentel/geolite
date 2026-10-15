use std::time::Instant;

use rusqlite::Connection;

use crate::database::store;
use crate::interfaces::cli::exec::stages::{stage, stage_context};
use crate::osm_pbf_file::osm_pbf_file;

fn inside(data_path: &str, path: &str) -> bool {
  let canonical = |p: &str| std::fs::canonicalize(p).unwrap_or_else(|_| std::path::PathBuf::from(p));
  canonical(path).starts_with(canonical(data_path))
}

// the pbf and the osm_data of one source, once the bases hold what the source brought
pub fn command_handler_optimize_delete_intermediary_data(data_path: &str, source: &str) -> bool {
  let start = Instant::now();
  let catalog = store::open_catalog(data_path);
  let Some(resolved) = crate::interfaces::cli::extract::resolve_input(&catalog, data_path, 0, source)
  else {
    std::process::exit(1);
  };
  let mut deleted = 0u32;
  {
    let conn = store::open_source(data_path, &resolved.stem);
    let opened: Vec<Connection> = store::base_pool::new(data_path)
      .on_disk()
      .iter()
      .map(|base| store::open_base_readonly(&base.sqlite))
      .collect();
    stage::optimize_delete_intermediary_data.require(&stage_context {
      catalog: Some(&conn),
      file: Some(&resolved),
      bases: opened.iter().collect(),
    });
    // a pbf the user keeps outside the data path is the user's: only one downloaded or copied
    // into the data path is deleted with its chunks
    if inside(data_path, &resolved.path) {
      let bytes = osm_pbf_file::open(Some(&conn), data_path).delete(&resolved.path);
      deleted += 1;
      println!(
        "\x1b[1;32mdeleted\x1b[0m {}  {}",
        resolved.name,
        super::fmt_size(bytes)
      );
    }
  }
  let bytes = store::remove_osm_data_files(data_path, &resolved.stem);
  deleted += 1;
  println!(
    "\x1b[1;32mdeleted\x1b[0m {}.osm_data.sqlite3  {}",
    resolved.stem,
    super::fmt_size(bytes)
  );
  let total = start.elapsed().as_secs_f64();
  println!("\x1b[1;32mdeleted\x1b[0m {deleted} tables in {total:.1}s");
  true
}
