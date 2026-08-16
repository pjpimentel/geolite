// http stubs shared by every test that talks to a local server: a json endpoint (with optional
// request recording) and a range-aware file server mirroring geofabrik's download behavior.
// single home for these helpers — duplicating them per test file trips sonar's duplication gate.
use std::{
  io::{Read, Write},
  net::{TcpListener, TcpStream},
  sync::{Arc, Mutex},
};

pub(crate) enum md5_reply {
  not_found,
  // 200 with "{hash}  file.osm.pbf\n"
  hash(String),
  // 200 with verbatim body bytes
  raw(Vec<u8>),
}

pub(crate) fn start_json_server(body: String) -> String {
  start_recording_json_server(body).0
}

// serves the json index and records the raw text of every request received, so
// scenarios can assert on request count and on request headers.
pub(crate) fn start_recording_json_server(body: String) -> (String, Arc<Mutex<Vec<String>>>) {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let port = listener.local_addr().unwrap().port();
  let url = format!("http://127.0.0.1:{port}/index.json");
  let body = Arc::new(body);
  let requests: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
  let requests_srv = requests.clone();
  std::thread::spawn(move || {
    for stream in listener.incoming().flatten() {
      let body = Arc::clone(&body);
      let requests = Arc::clone(&requests_srv);
      std::thread::spawn(move || serve_json(stream, &body, &requests));
    }
  });
  (url, requests)
}

fn serve_json(mut stream: TcpStream, body: &str, requests: &Mutex<Vec<String>>) {
  let mut buf = [0u8; 4096];
  let n = stream.read(&mut buf).unwrap_or(0);
  // records before responding, so a returned call always sees its own request.
  requests
    .lock()
    .unwrap()
    .push(String::from_utf8_lossy(&buf[..n]).to_string());
  let response = format!(
    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
    body.len(),
    body
  );
  let _ = stream.write_all(response.as_bytes());
}

pub(crate) fn start_file_server(content: Vec<u8>, md5: md5_reply) -> String {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let port = listener.local_addr().unwrap().port();
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
      md5_reply::raw(bytes) => {
        let header = format!(
          "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
          bytes.len()
        );
        stream.write_all(header.as_bytes()).ok();
        stream.write_all(bytes).ok();
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
