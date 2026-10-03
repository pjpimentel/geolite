use std::io::IsTerminal;

use crate::interfaces::tui::tree::{self, opened};
use crate::interfaces::tui::{listing, view};

pub fn command_handler_tui(sqlite_path: &str, path: Option<&str>) {
  let conn = require_data(sqlite_path);
  let tree = tree::open(&conn);
  let opened = match tree.resolve(&conn, path.unwrap_or("")) {
    Ok(opened) => opened,
    Err(message) => {
      eprintln!("\x1b[1;31merror\x1b[0m: {message}");
      std::process::exit(1);
    }
  };
  if std::io::stdout().is_terminal() {
    view::run(&conn, tree, opened);
    return;
  }
  match opened {
    opened::folder(folder) => listing::print(&folder),
    opened::leaf(_, leaf) => listing::print_leaf(&leaf),
  }
}

fn require_data(sqlite_path: &str) -> rusqlite::Connection {
  if std::path::Path::new(sqlite_path).exists() {
    let conn = crate::database::open_readonly(sqlite_path);
    if crate::admin_level_hierarchy::repository::count(&conn) > 0 {
      return conn;
    }
  }
  eprintln!("\x1b[1;31merror\x1b[0m: first you need to build the data using geolite build");
  std::process::exit(1);
}
