pub mod delete_intermediary_data;
pub mod merge_admin_levels;
pub mod sqlite_file;

use crate::interfaces::cli::exec::stages::stage;

fn file_size(path: &str) -> u64 {
  std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

fn dir_size(path: &str) -> u64 {
  let p = std::path::Path::new(path);
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

fn print_sqlite_sizes(label: &str, sqlite_path: &str, index_path: &str) {
  let main = file_size(sqlite_path)
    + file_size(&format!("{sqlite_path}-wal"))
    + file_size(&format!("{sqlite_path}-shm"));
  let osm_path = crate::database::osm_data_path(sqlite_path);
  let osm = file_size(&osm_path)
    + file_size(&format!("{osm_path}-wal"))
    + file_size(&format!("{osm_path}-shm"));
  let index = dir_size(index_path);
  println!(
    "\x1b[1;32m{label}\x1b[0m  main: {}  osm_data: {}  index: {}  total: {}",
    fmt_size(main),
    fmt_size(osm),
    fmt_size(index),
    fmt_size(main + osm + index),
  );
}

pub(super) fn with_sqlite_sizes(sqlite_path: &str, index_path: &str, stage: impl FnOnce()) {
  print_sqlite_sizes("before", sqlite_path, index_path);
  println!();
  stage();
  println!();
  print_sqlite_sizes("after ", sqlite_path, index_path);
}

pub fn command_handler_optimize(data_path: &str, sqlite_path: &str, index_path: &str) {
  print_sqlite_sizes("before", sqlite_path, index_path);
  println!();
  if !delete_intermediary_data::command_handler_optimize_delete_intermediary_data(
    data_path,
    sqlite_path,
  ) {
    return;
  }
  println!();
  println!("\x1b[2m── {}\x1b[0m", stage::optimize_sqlite_file.name());
  sqlite_file::command_handler_optimize_sqlite_file(sqlite_path);
  println!();
  print_sqlite_sizes("after ", sqlite_path, index_path);
}
