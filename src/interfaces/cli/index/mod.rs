pub mod admin_levels_hierarchy;
pub mod coordinates;
pub mod user_friendly_name;

use crate::interfaces::cli::exec::stages::stage;
use crate::interfaces::cli::optimize::merge_admin_levels;

pub fn command_handler_index_and_merge(
  sqlite_path: &str,
  index_path: &str,
  preset: &crate::presets::index_user_friendly_name_preset,
) {
  admin_levels_hierarchy::command_handler_index_admin_levels_hierarchy(sqlite_path);
  println!();
  println!(
    "\x1b[2m── {}\x1b[0m",
    stage::optimize_merge_admin_levels.name()
  );
  merge_admin_levels::command_handler_optimize_merge_admin_levels(sqlite_path, index_path);
  println!();
  println!("\x1b[2m── {}\x1b[0m", stage::index_user_friendly_name.name());
  user_friendly_name::command_handler_index_user_friendly_name(sqlite_path, index_path, preset);
  println!();
  println!("\x1b[2m── {}\x1b[0m", stage::index_coordinates.name());
  coordinates::command_handler_index_coordinates(sqlite_path);
}
