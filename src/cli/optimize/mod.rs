pub mod delete_intermediary_data;
pub mod sqlite_file;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum optimize_commands {
  #[command(name = "delete-intermediary-data")]
  delete_intermediary_data,
  #[command(name = "sqlite-file")]
  sqlite_file { pbf_or_sqlite: String },
}

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

pub fn command_handler_optimize(
  data_path: &str,
  sqlite_path: &str,
  index_path: &str,
  command: Option<optimize_commands>,
) {
  print_sqlite_sizes("before", sqlite_path, index_path);
  println!();
  match command {
    None => {
      use std::cell::Cell;
      crate::cli::require_sqlite(sqlite_path);
      {
        let conn = crate::database::open_write(sqlite_path);
        if crate::database::admin_levels::count_with_geometry(&conn) == 0 {
          eprintln!("\x1b[1;31merror\x1b[0m: admin_levels is empty — run extract first");
          return;
        }
        if crate::database::admin_levels_hierarchy::count(&conn) == 0 {
          eprintln!("\x1b[1;31merror\x1b[0m: admin_levels_hierarchy is empty — run index first");
          return;
        }
      }
      let delete_start = std::time::Instant::now();
      let table_count = Cell::new(0u32);
      crate::optimize::delete_intermediary_data::run(data_path, sqlite_path, |name, bytes| {
        table_count.set(table_count.get() + 1);
        println!("\x1b[1;32mdeleted\x1b[0m {name}  {}", fmt_size(bytes));
      });
      let delete_total = delete_start.elapsed().as_secs_f64();
      let n = table_count.get();
      println!("\x1b[1;32mdeleted\x1b[0m {n} tables in {delete_total:.1}s");
      println!();
      {
        use std::io::Write;
        print!("\x1b[1;32moptimizing\x1b[0m sqlite file...");
        let _ = std::io::stdout().flush();
        let start = std::time::Instant::now();
        let conn = crate::database::open_write_main(sqlite_path);
        let (bytes_before, bytes_after) = crate::optimize::sqlite_file::run(&conn);
        println!(
          "\n\x1b[1;32moptimized\x1b[0m sqlite in {:.1}s  {} → {}",
          start.elapsed().as_secs_f64(),
          fmt_size(bytes_before),
          fmt_size(bytes_after),
        );
      }
    }
    Some(optimize_commands::delete_intermediary_data) => {
      delete_intermediary_data::command_handler_optimize_delete_intermediary_data(
        data_path,
        sqlite_path,
      )
    }
    Some(optimize_commands::sqlite_file { .. }) => {
      sqlite_file::command_handler_optimize_sqlite_file(sqlite_path)
    }
  }
  println!();
  print_sqlite_sizes("after ", sqlite_path, index_path);
}
