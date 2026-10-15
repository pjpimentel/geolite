pub mod addresses;
pub mod admin_levels_hierarchy;
pub mod coordinates;

use crate::database::store::base_files;
use crate::interfaces::cli::exec::stages::stage;
use crate::interfaces::cli::optimize::{delete_isolated_admin_levels, merge_admin_levels};

// the derived artifacts of one base, rebuilt from its rows in the order the pipeline runs them; a
// base the isolated rows leave empty is removed by that stage, and `false` says so
pub fn index_base(base: &base_files) -> bool {
  admin_levels_hierarchy::command_handler_index_admin_levels_hierarchy(&base.sqlite);
  println!();
  println!(
    "\x1b[2m── {}\x1b[0m",
    stage::optimize_delete_isolated_admin_levels.name()
  );
  delete_isolated_admin_levels::command_handler_optimize_delete_isolated_admin_levels(
    &base.sqlite,
    &base.index,
  );
  if !std::path::Path::new(&base.sqlite).exists() {
    return false;
  }
  println!();
  println!(
    "\x1b[2m── {}\x1b[0m",
    stage::optimize_merge_admin_levels.name()
  );
  merge_admin_levels::command_handler_optimize_merge_admin_levels(&base.sqlite, &base.index);
  println!();
  println!("\x1b[2m── {}\x1b[0m", stage::index_addresses.name());
  addresses::command_handler_index_addresses(&base.sqlite, &base.index);
  println!();
  println!("\x1b[2m── {}\x1b[0m", stage::index_coordinates.name());
  coordinates::command_handler_index_coordinates(&base.sqlite);
  true
}

// the bases that are still there after their indexes
pub fn index_bases(bases: &[base_files]) -> Vec<base_files> {
  let mut kept: Vec<base_files> = Vec::new();
  for (i, base) in bases.iter().enumerate() {
    if i > 0 {
      println!();
      println!("\x1b[2m── {}\x1b[0m", stage::index_admin_levels_hierarchy.name());
    }
    println!("\x1b[2mbase {}\x1b[0m", base.file_name());
    if index_base(base) {
      kept.push(base.clone());
    }
  }
  kept
}
