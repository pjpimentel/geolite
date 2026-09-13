use std::path::Path;

use crate::domain::address::{address, query_input, query_opts};

pub fn command_handler_query(
  sqlite_path: &str,
  index_path: &str,
  text: &str,
  opts: query_opts,
  boosts: crate::domain::admin_level_hierarchy::tantivy_boosts,
  house_numbers: crate::domain::house_number::house_number_policy,
) {
  crate::cli::require_sqlite(sqlite_path);
  let conn = crate::database::open_readonly(sqlite_path);
  let index = match crate::domain::admin_level_hierarchy::search_index::load(
    Path::new(index_path),
    boosts,
  ) {
    Some(i) => i,
    None => {
      eprintln!(
        "\x1b[1;31merror\x1b[0m: tantivy index not found at {index_path} — run `geolite index user-friendly-name` first"
      );
      std::process::exit(1);
    }
  };
  let address = address::open(&conn, Some(&index), &house_numbers);
  let result = match query_input::parse(text) {
    query_input::coordinates {
      latitude,
      longitude,
    } => address.query_by_coordinates(latitude, longitude, &opts),
    query_input::text => address.query_by_text(text, &opts),
  };
  println!("{}", serde_json::to_string_pretty(&result).unwrap());
}

#[cfg(test)]
#[path = "query.test.rs"]
mod tests;
