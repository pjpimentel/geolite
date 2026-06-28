use std::{
  fs,
  io::{Read, Write},
  net::{TcpListener, TcpStream},
  path::PathBuf,
  sync::Arc,
};

use super::{download_event, md5_status, run};

fn tmp(name: &str) -> PathBuf {
  let p = std::env::temp_dir().join(name);
  let _ = fs::remove_dir_all(&p);
  fs::create_dir_all(&p).unwrap();
  p
}

fn start_file_server(content: Vec<u8>, md5_hash: Option<String>) -> String {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let port = listener.local_addr().unwrap().port();
  let url = format!("http://127.0.0.1:{port}/file.osm.pbf");
  let content = Arc::new(content);
  std::thread::spawn(move || {
    for stream in listener.incoming().flatten() {
      let content = Arc::clone(&content);
      let md5_hash = md5_hash.clone();
      std::thread::spawn(move || handle_connection(stream, &content, md5_hash.as_deref()));
    }
  });
  url
}

fn handle_connection(mut stream: TcpStream, content: &[u8], md5_hash: Option<&str>) {
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
    match md5_hash {
      None => {
        stream
          .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
          .ok();
      }
      Some(hash) => {
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
  match method {
    "HEAD" => {
      let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
        content.len()
      );
      stream.write_all(response.as_bytes()).ok();
    }
    "GET" => {
      if let Some(range_str) = range {
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
      } else {
        let response = format!(
          "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
          content.len()
        );
        stream.write_all(response.as_bytes()).ok();
        stream.write_all(content).ok();
      }
    }
    _ => {
      stream
        .write_all(
          b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )
        .ok();
    }
  }
}

// 00: downloads file from url and saves to data_path root.
#[test]
fn _00_downloads_file_to_data_path_root() {
  let dir = tmp("dl_t00");
  let content = b"fake pbf content";
  let url = start_file_server(content.to_vec(), None);
  let out = run(dir.to_str().unwrap(), &url, 1, |_| {});
  assert!(out.is_some());
  assert_eq!(fs::read(dir.join("file.osm.pbf")).unwrap(), content);
}

// 01: reports md5_status::ok when checksum matches.
#[test]
fn _01_reports_md5_ok_when_checksum_matches() {
  let dir = tmp("dl_t01");
  let content = b"fake pbf content";
  let hash = format!("{:x}", md5::compute(content));
  let url = start_file_server(content.to_vec(), Some(hash));
  let out = run(dir.to_str().unwrap(), &url, 1, |_| {}).unwrap();
  assert!(matches!(out.md5, md5_status::ok));
}

// 02: reports md5_status::unavailable when .md5 endpoint returns 404.
#[test]
fn _02_reports_md5_unavailable_when_md5_endpoint_is_not_found() {
  let dir = tmp("dl_t02");
  let content = b"fake pbf content";
  let url = start_file_server(content.to_vec(), None);
  let out = run(dir.to_str().unwrap(), &url, 1, |_| {}).unwrap();
  assert!(matches!(out.md5, md5_status::unavailable));
}

// 03: reports md5_status::mismatch without panicking when hash does not match.
#[test]
fn _03_reports_md5_mismatch_without_panicking() {
  let dir = tmp("dl_t03");
  let content = b"fake pbf content";
  let url = start_file_server(
    content.to_vec(),
    Some("deadbeefdeadbeefdeadbeefdeadbeef".to_string()),
  );
  let out = run(dir.to_str().unwrap(), &url, 1, |_| {}).unwrap();
  assert!(matches!(out.md5, md5_status::mismatch { .. }));
}

// 04: parallel download reassembles chunks in order.
#[test]
fn _04_parallel_download_reassembles_chunks_in_order() {
  let dir = tmp("dl_t04");
  let content: Vec<u8> = (0u8..=255).collect();
  let url = start_file_server(content.clone(), None);
  let out = run(dir.to_str().unwrap(), &url, 4, |_| {});
  assert!(out.is_some());
  assert_eq!(fs::read(dir.join("file.osm.pbf")).unwrap(), content);
}

// 05: reuses existing file (does not redownload) and emits file_already_exists.
#[test]
fn _05_reuses_existing_file_without_redownloading() {
  use std::sync::Mutex;
  let dir = tmp("dl_t05");
  let existing = b"pre-existing content";
  fs::write(dir.join("file.osm.pbf"), existing).unwrap();
  let url = start_file_server(b"new content".to_vec(), None);
  let got_event: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
  let got_cb = got_event.clone();
  let out = run(dir.to_str().unwrap(), &url, 1, move |e| {
    if let download_event::file_already_exists { .. } = e {
      *got_cb.lock().unwrap() = true;
    }
  });
  let output = out.expect("expected reuse to return download_output");
  assert!(*got_event.lock().unwrap());
  assert_eq!(output.path, dir.join("file.osm.pbf"));
  assert_eq!(output.total_bytes, existing.len() as u64);
  assert_eq!(fs::read(dir.join("file.osm.pbf")).unwrap(), existing);
}

// 06: merge_progress events are emitted with cumulative byte counts during merge.
#[test]
fn _06_merge_progress_events_emitted_during_merge() {
  use std::sync::Mutex;
  let dir = tmp("dl_t06");
  let content: Vec<u8> = (0u8..=255).collect();
  let url = start_file_server(content.clone(), None);
  let progress: Arc<Mutex<Vec<(u64, u64)>>> = Arc::new(Mutex::new(vec![]));
  let prog_cb = progress.clone();
  run(dir.to_str().unwrap(), &url, 2, move |e| {
    if let download_event::merge_progress { done, total } = e {
      prog_cb.lock().unwrap().push((done, total));
    }
  });
  let events = progress.lock().unwrap();
  assert_eq!(events.len(), 2);
  assert_eq!(events[1].0, events[1].1);
}
