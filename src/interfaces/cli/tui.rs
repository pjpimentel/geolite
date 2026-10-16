use std::io::IsTerminal;

use crate::database::store::base_files;
use crate::interfaces::tui::tree::{self, opened};
use crate::interfaces::tui::{listing, view};

pub fn command_handler_tui(data_path: &str, single: Option<&base_files>, path: Option<&str>) {
  let bases: Vec<(String, rusqlite::Connection)> = crate::interfaces::cli::served_bases(data_path, single)
    .iter()
    .map(|base| (base.iso.to_ascii_uppercase(), require_data(&base.sqlite)))
    .collect();
  let tree = tree::open(bases);
  let opened = match tree.resolve(path.unwrap_or("")) {
    Ok(opened) => opened,
    Err(message) => {
      eprintln!("\x1b[1;31merror\x1b[0m: {message}");
      std::process::exit(1);
    }
  };
  if std::io::stdout().is_terminal() {
    view::run(tree, opened);
    return;
  }
  match opened {
    opened::folder(folder) => listing::print(&folder),
    opened::leaf(_, leaf) => listing::print_leaf(&leaf),
  }
}

fn require_data(sqlite_path: &str) -> rusqlite::Connection {
  if std::path::Path::new(sqlite_path).exists() {
    let conn = crate::database::store::open_base_immutable(sqlite_path);
    if crate::admin_level_hierarchy::repository::count(&conn) > 0 {
      return conn;
    }
  }
  eprintln!("\x1b[1;31merror\x1b[0m: first you need to build the data using geolite build");
  std::process::exit(1);
}
