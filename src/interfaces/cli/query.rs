use std::path::Path;

use crate::address::{address, query_input, query_opts};

pub fn command_handler_query(
  sqlite_path: &str,
  index_path: &str,
  text: &str,
  opts: query_opts,
  preset: &crate::presets::preset,
) {
  crate::interfaces::cli::require_sqlite(sqlite_path);
  let conn = crate::database::open_readonly(sqlite_path);
  let index = match crate::admin_level_hierarchy::search_index::load(
    Path::new(index_path),
    preset.index_user_friendly_name.boosts,
  ) {
    Some(i) => i,
    None => {
      eprintln!(
        "\x1b[1;31merror\x1b[0m: tantivy index not found at {index_path} — run `geolite index user-friendly-name` first"
      );
      std::process::exit(1);
    }
  };
  let address = address::open(&conn, Some(&index), &preset.house_numbers);
  let result = match query_input::parse(text) {
    query_input::coordinates {
      latitude,
      longitude,
    } => address.query_by_coordinates(latitude, longitude, &opts),
    query_input::text => address.query_by_text(text, &opts),
  };
  println!("{}", serde_json::to_string_pretty(&result).unwrap());
}
