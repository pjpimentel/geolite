pub mod admin_levels_hierarchy;
pub mod coordinates;
pub mod user_friendly_name;

use clap::Subcommand;
use indicatif::ProgressBar;

// `cargo test` runs with a tty on stderr, where indicatif draws outside libtest's output capture and
// would pollute the test log. hide the index progress bars under test; the real cli keeps them live.
fn progress_bar() -> ProgressBar {
  if cfg!(test) {
    ProgressBar::hidden()
  } else {
    ProgressBar::new_spinner()
  }
}

#[derive(Subcommand)]
pub enum index_commands {
  #[command(name = "admin-levels-hierarchy")]
  admin_levels_hierarchy,
  #[command(name = "user-friendly-name")]
  user_friendly_name,
  #[command(name = "coordinates")]
  coordinates,
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
      user_friendly_name::command_handler_index_user_friendly_name(sqlite_path, index_path, preset);
      println!();
      coordinates::command_handler_index_coordinates(sqlite_path);
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
