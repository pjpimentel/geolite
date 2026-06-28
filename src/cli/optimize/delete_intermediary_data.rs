pub fn command_handler_optimize_delete_intermediary_data(data_path: &str, sqlite_path: &str) {
  use std::cell::Cell;
  use std::time::Instant;
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
  let start = Instant::now();
  let table_count = Cell::new(0u32);
  crate::optimize::delete_intermediary_data::run(data_path, sqlite_path, |name, bytes| {
    table_count.set(table_count.get() + 1);
    println!("\x1b[1;32mdeleted\x1b[0m {name}  {}", super::fmt_size(bytes));
  });
  let total = start.elapsed().as_secs_f64();
  let n = table_count.get();
  println!("\x1b[1;32mdeleted\x1b[0m {n} tables in {total:.1}s");
}
