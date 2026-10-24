pub mod delete_intermediary_data;
pub mod delete_isolated_admin_levels;
pub mod merge_admin_levels;
pub mod sqlite_file;

use std::io::Write;
use std::path::Path;

use rusqlite::Connection;

use crate::database::store::{self, base_files};
use crate::interfaces::cli::exec::stages::stage;

fn clear_indexes(conn: &Connection, index_path: &str) {
  print!("\x1b[1;32mclearing\x1b[0m addresses and coordinates...");
  let _ = std::io::stdout().flush();
  crate::admin_level_hierarchy::search_index::destroy(Path::new(index_path));
  crate::admin_level::spatial_index::recreate(conn);
  println!(" done");
}

fn file_size(path: &str) -> u64 {
  std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn dir_size(path: &str) -> u64 {
  let p = Path::new(path);
  let Ok(entries) = std::fs::read_dir(p) else {
    return 0;
  };
  let mut total = 0u64;
  for entry in entries.flatten() {
    let Ok(metadata) = entry.metadata() else {
      continue;
    };
    if metadata.is_file() {
      total += metadata.len();
    } else if metadata.is_dir() {
      total += dir_size(&entry.path().to_string_lossy());
    }
  }
  total
}

fn fmt_size(bytes: u64) -> String {
  if bytes >= 1_073_741_824 {
    format!("{:.2} gb", bytes as f64 / 1_073_741_824.0)
  } else if bytes >= 1_048_576 {
    format!("{:.2} mb", bytes as f64 / 1_048_576.0)
  } else if bytes >= 1_024 {
    format!("{:.2} kb", bytes as f64 / 1_024.0)
  } else {
    format!("{bytes} b")
  }
}

fn files_size(path: &str) -> u64 {
  file_size(path) + file_size(&format!("{path}-wal")) + file_size(&format!("{path}-shm"))
}

// what the data path weighs: the osm_data, and every base with its index
pub(super) fn print_sizes(label: &str, data_path: &str) {
  let osm_data = files_size(&store::osm_data_path(data_path));
  let (mut bases, mut indexes) = (0u64, 0u64);
  for base in store::bases(data_path) {
    bases += files_size(&base.sqlite);
    indexes += dir_size(&base.index);
  }
  println!(
    "\x1b[1;32m{label}\x1b[0m  osm_data: {}  bases: {}  indexes: {}  total: {}",
    fmt_size(osm_data),
    fmt_size(bases),
    fmt_size(indexes),
    fmt_size(osm_data + bases + indexes),
  );
}

pub(super) fn with_sizes(data_path: &str, stage: impl FnOnce()) {
  print_sizes("before", data_path);
  println!();
  stage();
  println!();
  print_sizes("after ", data_path);
}

pub fn command_handler_optimize(data_path: &str, bases: &[base_files]) {
  print_sizes("before", data_path);
  println!();
  if !delete_intermediary_data::command_handler_optimize_delete_intermediary_data(data_path) {
    return;
  }
  for base in bases {
    println!();
    println!("\x1b[2m── {} {}\x1b[0m", stage::optimize_sqlite_file.name(), base.label());
    sqlite_file::command_handler_optimize_sqlite_file(&base.sqlite);
  }
  println!();
  print_sizes("after ", data_path);
}
