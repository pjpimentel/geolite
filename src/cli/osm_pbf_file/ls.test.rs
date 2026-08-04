use super::command_handler_osm_pbf_file_ls;
use crate::cli::osm_pbf_file::osm_pbf_file_ls_source;
use crate::extract::pbf_fixtures::tempdir_guard;
use std::io::{Read, Write};

// serves a fixed json body on every request — the geofabrik index stub.
fn start_json_server(body: &'static str) -> String {
  let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("failed to bind stub server");
  let port = listener.local_addr().expect("stub server addr").port();
  std::thread::spawn(move || {
    for mut stream in listener.incoming().flatten() {
      let mut buf = [0u8; 4096];
      let _ = stream.read(&mut buf);
      let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
      );
      let _ = stream.write_all(response.as_bytes());
    }
  });
  format!("http://127.0.0.1:{port}/index.json")
}

const TWO_FEATURES: &str = r#"{"features":[
  {"properties":{"id":"alpha","name":"Alpha","urls":{"pbf":"https://example.invalid/alpha.osm.pbf"}}},
  {"properties":{"id":"beta","name":"Beta","parent":"alpha"}}
]}"#;

#[test]
fn _00_00_prints_the_geofabrik_table_from_the_stub_index() {
  let guard = tempdir_guard::new("cli_ls_geofabrik");
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  let endpoint = start_json_server(TWO_FEATURES);
  command_handler_osm_pbf_file_ls(
    &guard.path.to_string_lossy(),
    &db,
    &osm_pbf_file_ls_source::geofabrik,
    &endpoint,
    &false,
  );
}

#[test]
fn _00_01_prints_the_header_only_for_an_empty_index() {
  let guard = tempdir_guard::new("cli_ls_geofabrik_empty");
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  let endpoint = start_json_server(r#"{"features":[]}"#);
  command_handler_osm_pbf_file_ls(
    &guard.path.to_string_lossy(),
    &db,
    &osm_pbf_file_ls_source::geofabrik,
    &endpoint,
    &true,
  );
}

#[test]
fn _01_00_lists_local_pbf_files() {
  let guard = tempdir_guard::new("cli_ls_local");
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  std::fs::write(guard.path.join("a.osm.pbf"), b"aa").expect("failed to write pbf");
  std::fs::write(guard.path.join("b.osm.pbf"), b"bbbb").expect("failed to write pbf");
  command_handler_osm_pbf_file_ls(
    &guard.path.to_string_lossy(),
    &db,
    &osm_pbf_file_ls_source::local,
    "http://127.0.0.1:1/unused.json",
    &false,
  );
}

#[test]
fn _01_01_prints_a_message_when_there_are_no_local_pbf_files() {
  let guard = tempdir_guard::new("cli_ls_local_empty");
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  command_handler_osm_pbf_file_ls(
    &guard.path.to_string_lossy(),
    &db,
    &osm_pbf_file_ls_source::local,
    "http://127.0.0.1:1/unused.json",
    &false,
  );
}
