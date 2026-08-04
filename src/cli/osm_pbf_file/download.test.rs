use super::command_handler_osm_pbf_file_download;
use crate::extract::pbf_fixtures::tempdir_guard;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

enum md5_reply {
  not_found,
  // 200 with "{hash}  file.osm.pbf\n"
  hash(&'static str),
}

// same shape as the stub in src/osm_pbf_file/download.test.rs: HEAD announces the size, GET
// honours range requests, and the .md5 sibling answers per `md5`.
fn start_file_server(content: Vec<u8>, md5: md5_reply) -> String {
  let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind stub server");
  let port = listener.local_addr().expect("stub server addr").port();
  let url = format!("http://127.0.0.1:{port}/file.osm.pbf");
  let content = Arc::new(content);
  let md5 = Arc::new(md5);
  std::thread::spawn(move || {
    for stream in listener.incoming().flatten() {
      let content = Arc::clone(&content);
      let md5 = Arc::clone(&md5);
      std::thread::spawn(move || handle_connection(stream, &content, &md5));
    }
  });
  url
}

fn handle_connection(mut stream: TcpStream, content: &[u8], md5: &md5_reply) {
  let mut buf = [0u8; 4096];
  let n = stream.read(&mut buf).unwrap_or(0);
  let request = String::from_utf8_lossy(&buf[..n]);
  let first_line = request.lines().next().unwrap_or("");
  let mut parts = first_line.split_whitespace();
  let method = parts.next().unwrap_or("");
  let path = parts.next().unwrap_or("");
  let range = request
    .lines()
    .find(|l| l.to_lowercase().starts_with("range:"))
    .and_then(|l| l.split_once(':').map(|x| x.1))
    .map(|v| v.trim().to_string());
  if path.ends_with(".md5") {
    match md5 {
      md5_reply::not_found => {
        stream
          .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
          .ok();
      }
      md5_reply::hash(hash) => {
        let body = format!("{hash}  file.osm.pbf\n");
        let response = format!(
          "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
          body.len()
        );
        stream.write_all(response.as_bytes()).ok();
      }
    }
    return;
  }
  if method == "HEAD" {
    let response = format!(
      "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
      content.len()
    );
    stream.write_all(response.as_bytes()).ok();
  } else if method == "GET" {
    let range_str = range.unwrap_or_default();
    let range_str = range_str.trim_start_matches("bytes=");
    let mut iter = range_str.splitn(2, '-');
    let start: usize = iter.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let end: usize = iter
      .next()
      .and_then(|s| s.parse().ok())
      .unwrap_or(content.len() - 1)
      .min(content.len() - 1);
    let chunk = &content[start..=end];
    let response = format!(
      "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
      content.len(),
      chunk.len()
    );
    stream.write_all(response.as_bytes()).ok();
    stream.write_all(chunk).ok();
  }
}

fn start_json_server(body: &'static str) -> String {
  let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind stub server");
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

fn scene(tag: &str) -> (tempdir_guard, String, String) {
  let guard = tempdir_guard::new(tag);
  let data = guard.path.to_string_lossy().into_owned();
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  (guard, data, db)
}

#[test]
fn _00_00_downloads_a_url_with_two_threads_and_saves_it() {
  let (guard, data, db) = scene("cli_download_url");
  let url = start_file_server(b"fake pbf content".to_vec(), md5_reply::not_found);
  command_handler_osm_pbf_file_download(
    &data,
    &2,
    &db,
    &[url],
    "http://127.0.0.1:1/unused.json",
    false,
  );
  let saved = guard.path.join("file.osm.pbf");
  assert_eq!(
    std::fs::read(&saved).expect("downloaded file must exist"),
    b"fake pbf content"
  );
}

#[test]
fn _00_01_warns_on_md5_mismatch_but_keeps_the_file() {
  let (guard, data, db) = scene("cli_download_md5");
  let url = start_file_server(
    b"fake pbf content".to_vec(),
    md5_reply::hash("00000000000000000000000000000000"),
  );
  command_handler_osm_pbf_file_download(
    &data,
    &1,
    &db,
    &[url],
    "http://127.0.0.1:1/unused.json",
    false,
  );
  assert!(guard.path.join("file.osm.pbf").exists());
}

#[test]
fn _00_02_reuses_an_existing_file_without_downloading() {
  let (guard, data, db) = scene("cli_download_reuse");
  std::fs::write(guard.path.join("file.osm.pbf"), b"already here").expect("failed to pre-create");
  let url = start_file_server(b"other content".to_vec(), md5_reply::not_found);
  command_handler_osm_pbf_file_download(
    &data,
    &1,
    &db,
    &[url],
    "http://127.0.0.1:1/unused.json",
    false,
  );
  assert_eq!(
    std::fs::read(guard.path.join("file.osm.pbf")).expect("file must remain"),
    b"already here",
    "an existing file must be reused, not overwritten"
  );
}

#[test]
fn _00_03_resolves_a_geofabrik_id_through_the_stub_index() {
  let (guard, data, db) = scene("cli_download_resolve");
  let url = start_file_server(b"tiny region".to_vec(), md5_reply::not_found);
  let body: &'static str = Box::leak(
    format!(r#"{{"features":[{{"properties":{{"id":"tiny","name":"Tiny","urls":{{"pbf":"{url}"}}}}}}]}}"#)
      .into_boxed_str(),
  );
  let endpoint = start_json_server(body);
  command_handler_osm_pbf_file_download(&data, &1, &db, &["tiny".to_string()], &endpoint, false);
  assert!(guard.path.join("file.osm.pbf").exists());
}

#[test]
fn _00_04_continues_past_an_unknown_id_when_not_aborting() {
  let (guard, data, db) = scene("cli_download_continue");
  let url = start_file_server(b"fake pbf content".to_vec(), md5_reply::not_found);
  let endpoint = start_json_server(r#"{"features":[]}"#);
  command_handler_osm_pbf_file_download(
    &data,
    &1,
    &db,
    &["nope".to_string(), url],
    &endpoint,
    false,
  );
  assert!(
    guard.path.join("file.osm.pbf").exists(),
    "the second input must still be downloaded"
  );
}

#[test]
#[ignore] // executed only as a child of _01_00
fn _90_download_unknown_id_with_abort() {
  let (_guard, data, db) = scene("cli_download_abort");
  let endpoint = start_json_server(r#"{"features":[]}"#);
  command_handler_osm_pbf_file_download(&data, &1, &db, &["nope".to_string()], &endpoint, true);
}

#[test]
fn _01_00_unknown_id_with_abort_on_any_error_exits_one() {
  let out = crate::cli::tests::respawn(
    "cli::osm_pbf_file::download::tests::_90_download_unknown_id_with_abort",
    &[],
    &[],
  );
  assert_eq!(out.status.code(), Some(1), "stderr: {}", crate::cli::tests::stderr_of(&out));
  assert!(
    crate::cli::tests::stderr_of(&out)
      .contains("not a valid url nor a known geofabrik id")
  );
}
