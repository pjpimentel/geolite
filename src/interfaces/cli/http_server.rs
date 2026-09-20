pub fn command_handler_http_server(
  sqlite_path: &str,
  index_path: &str,
  server: crate::interfaces::http::bound_server,
  threads: u8,
  preset: &crate::presets::preset,
) {
  crate::interfaces::cli::require_sqlite(sqlite_path);
  crate::interfaces::http::serve(server, sqlite_path, index_path, threads, preset);
}
