pub mod admin_levels_hierarchy;
pub mod coordinates;
pub mod user_friendly_name;

use clap::Subcommand;

use crate::interfaces::cli::optimize::merge_admin_levels;

#[derive(Subcommand)]
pub enum index_commands {
  #[command(name = "admin-levels-hierarchy")]
  admin_levels_hierarchy,
  #[command(name = "user-friendly-name")]
  user_friendly_name,
  #[command(name = "coordinates")]
  coordinates,
}

fn command_handler_index_search(
  sqlite_path: &str,
  index_path: &str,
  preset: &crate::presets::index_user_friendly_name_preset,
) {
  user_friendly_name::command_handler_index_user_friendly_name(sqlite_path, index_path, preset);
  println!();
  coordinates::command_handler_index_coordinates(sqlite_path);
}

pub fn command_handler_index_and_merge(
  sqlite_path: &str,
  index_path: &str,
  preset: &crate::presets::index_user_friendly_name_preset,
) {
  admin_levels_hierarchy::command_handler_index_admin_levels_hierarchy(sqlite_path);
  println!();
  println!("\x1b[2m── optimize merge-admin-levels\x1b[0m");
  merge_admin_levels::command_handler_optimize_merge_admin_levels(sqlite_path, index_path);
  println!();
  println!("\x1b[2m── index\x1b[0m");
  command_handler_index_search(sqlite_path, index_path, preset);
}

pub fn command_handler_index(
  sqlite_path: &str,
  index_path: &str,
  command: Option<index_commands>,
  preset: &crate::presets::index_user_friendly_name_preset,
) {
  match command {
    None => {
      admin_levels_hierarchy::command_handler_index_admin_levels_hierarchy(sqlite_path);
      println!();
      command_handler_index_search(sqlite_path, index_path, preset);
    }
    Some(index_commands::admin_levels_hierarchy) => {
      admin_levels_hierarchy::command_handler_index_admin_levels_hierarchy(sqlite_path)
    }
    Some(index_commands::user_friendly_name) => {
      user_friendly_name::command_handler_index_user_friendly_name(sqlite_path, index_path, preset)
    }
    Some(index_commands::coordinates) => {
      coordinates::command_handler_index_coordinates(sqlite_path)
    }
  }
}
