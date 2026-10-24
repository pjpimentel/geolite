use crate::database::store::base_files;

pub fn command_handler_http_server(
  data_path: &str,
  single: Option<&base_files>,
  server: crate::interfaces::http::bound_server,
  threads: u8,
) {
  let bases = crate::interfaces::cli::served_bases(data_path, single);
  crate::interfaces::http::serve(server, bases, threads);
}
