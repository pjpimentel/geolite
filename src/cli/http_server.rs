pub fn command_handler_http_server(
  sqlite_path: &str,
  index_path: &str,
  server: crate::http::bound_server,
  threads: u8,
  boosts: crate::admin_level_hierarchy::tantivy_boosts,
  house_numbers: crate::house_number::house_number_policy,
) {
  crate::cli::require_sqlite(sqlite_path);
  crate::http::serve(
    server,
    sqlite_path,
    index_path,
    threads,
    boosts,
    house_numbers,
  );
}
