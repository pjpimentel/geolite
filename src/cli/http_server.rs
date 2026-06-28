pub fn command_handler_http_server(
  sqlite_path: &str,
  index_path: &str,
  host: &str,
  port: u16,
  threads: u8,
  boosts: crate::index::admin_levels_hierarchy_tantivy::tantivy_boosts,
) {
  crate::cli::require_sqlite(sqlite_path);
  crate::http::serve(sqlite_path, index_path, host, port, threads, boosts);
}
