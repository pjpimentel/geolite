use std::path::Path;

#[allow(clippy::too_many_arguments)]
pub fn command_handler_query(
  sqlite_path: &str,
  index_path: &str,
  input: &str,
  friendly_name_format: Option<&str>,
  min_quality: Option<f64>,
  bounding_wkt: Option<crate::query::bounding_geometry>,
  last_admin_levels: Option<Vec<crate::domain::admin_level::level>>,
  include_wkt: bool,
  boosts: crate::domain::admin_level_hierarchy::tantivy_boosts,
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
  let result = crate::query::run(
    &conn,
    Some(&index),
    input,
    friendly_name_format,
    min_quality,
    bounding_wkt,
    last_admin_levels,
    include_wkt,
  );
  println!("{}", serde_json::to_string_pretty(&result).unwrap());
}

#[cfg(test)]
#[path = "query.test.rs"]
mod tests;
