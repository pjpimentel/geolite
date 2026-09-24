use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::admin_level::{level, level_error};
use crate::admin_level_hierarchy::tantivy_index;

mod openapi;
mod status;

const INDEX_HTML: &str = include_str!("static/index.html");

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

// SIGINT (ctrl-c) and SIGTERM (`docker stop`); these posix values are stable across linux/macos.
const SIGINT: std::os::raw::c_int = 2;
const SIGTERM: std::os::raw::c_int = 15;

// libc's signal(2), declared directly so no extra crate is pulled in for a one-flag handler.
unsafe extern "C" {
  fn signal(signum: std::os::raw::c_int, handler: extern "C" fn(std::os::raw::c_int)) -> usize;
}

extern "C" fn on_shutdown_signal(_signum: std::os::raw::c_int) {
  SHUTDOWN.store(true, Ordering::Relaxed);
}

pub struct bound_server {
  server: Arc<Server>,
  addr: SocketAddr,
}

impl bound_server {
  pub fn addr(&self) -> SocketAddr {
    self.addr
  }
}

pub fn bind(addr: &str) -> bound_server {
  let server = Server::http(addr).expect("failed to start http server");
  let addr = server
    .server_addr()
    .to_ip()
    .expect("the http server listens on a tcp socket");
  bound_server {
    server: Arc::new(server),
    addr,
  }
}

pub fn serve(
  bound: bound_server,
  sqlite_path: &str,
  index_path: &str,
  threads: u8,
  preset: &crate::presets::preset,
) {
  let addr = bound.addr();
  let server = bound.server;
  let sqlite_path = Arc::new(sqlite_path.to_string());

  // degraded boot: a missing index disables text_to_address (reported via /status) instead of
  // refusing to start, so the server keeps serving coordinate queries and the health endpoint.
  let boosts = preset.index_user_friendly_name.boosts;
  let index: Option<Arc<tantivy_index>> =
    crate::admin_level_hierarchy::search_index::load(Path::new(index_path), boosts).map(Arc::new);
  if index.is_none() {
    eprintln!(
      "\x1b[1;33mwarn\x1b[0m: tantivy index not found at {index_path} — text_to_address disabled; run `geolite exec index-user-friendly-name` to enable"
    );
  }

  let db_file = Arc::new(
    Path::new(sqlite_path.as_str())
      .file_name()
      .and_then(|s| s.to_str())
      .unwrap_or(sqlite_path.as_str())
      .to_string(),
  );

  // SAFETY: on_shutdown_signal only performs an atomic store, which is async-signal-safe.
  unsafe {
    signal(SIGINT, on_shutdown_signal);
    signal(SIGTERM, on_shutdown_signal);
  }

  // the e2e harness reads this line to learn the port a `--port 0` server took: keep its shape
  println!("\x1b[1;32mlistening\x1b[0m on {addr}  sqlite: {sqlite_path}  index: {index_path}");

  let handles: Vec<_> = (0..threads)
    .map(|_| {
      let server = server.clone();
      let sqlite_path = sqlite_path.clone();
      let index = index.clone();
      let db_file = db_file.clone();
      let preset = *preset;
      std::thread::spawn(move || {
        let conn = crate::database::open_readonly(&sqlite_path);
        // recv_timeout instead of the blocking incoming_requests() so each worker rechecks the
        // shutdown flag every poll interval (tiny_http's unblock() only wakes one thread).
        while !SHUTDOWN.load(Ordering::Relaxed) {
          match server.recv_timeout(Duration::from_millis(250)) {
            Ok(Some(request)) => handle(request, &conn, index.as_deref(), &db_file, &preset),
            Ok(None) => {}
            Err(_) => break,
          }
        }
      })
    })
    .collect();

  for h in handles {
    h.join().ok();
  }
  println!("\x1b[1;33mshutdown\x1b[0m: http server stopped");
}

fn handle(
  request: tiny_http::Request,
  conn: &rusqlite::Connection,
  index: Option<&tantivy_index>,
  db_file: &str,
  preset: &crate::presets::preset,
) {
  let start = Instant::now();
  let url = request.url().to_string();
  let method = request.method().clone();

  let (path, query_string) = match url.split_once('?') {
    Some((p, q)) => (p.to_string(), q.to_string()),
    None => (url.clone(), String::new()),
  };

  let known_route = matches!(
    path.as_str(),
    "/" | "/geocode" | "/status" | "/openapi.json" | "/docs"
  );

  if known_route && method == Method::Options {
    let response = force_content_length(
      Response::from_string("")
        .with_status_code(StatusCode(204))
        .with_header(cors_origin())
        .with_header(Header::from_bytes("Access-Control-Allow-Methods", "GET, OPTIONS").unwrap())
        .with_header(Header::from_bytes("Access-Control-Allow-Headers", "Content-Type").unwrap()),
    );
    request.respond(response).ok();
    log_access(&method, &url, 204, start.elapsed().as_millis());
    return;
  }

  if known_route && method != Method::Get {
    let status = respond_json(
      request,
      StatusCode(405),
      r#"{"error":"method not allowed"}"#,
    );
    log_access(&method, &url, status, start.elapsed().as_millis());
    return;
  }

  let status = match path.as_str() {
    "/" => respond_html(request, StatusCode(200), INDEX_HTML),
    "/status" => {
      let report = status::build(db_file, conn, index);
      let code = if report.is_ok {
        StatusCode(200)
      } else {
        StatusCode(503)
      };
      match serde_json::to_string(&report) {
        Ok(body) => respond_json(request, code, &body),
        Err(_) => respond_json(
          request,
          StatusCode(500),
          r#"{"error":"serialization error"}"#,
        ),
      }
    }
    "/openapi.json" => respond_json(request, StatusCode(200), openapi::spec_json()),
    "/docs" => respond_html(request, StatusCode(200), openapi::SWAGGER_HTML),
    "/geocode" => {
      let q = query_param(&query_string, "query");
      let friendly_name_format = query_param(&query_string, "friendly_name_format")
        .map(|s| crate::address::validate_friendly_name_format(&s))
        .transpose();
      let min_quality = query_param(&query_string, "quality")
        .map(|s| crate::address::parse_min_quality(&s))
        .transpose();
      let bounding_wkt = query_param(&query_string, "bounding_wkt")
        .map(|s| crate::admin_level::geometry::parse_bounding_wkt(&s))
        .transpose();
      let last_admin_levels = query_param(&query_string, "last_admin_levels")
        .map(|s| parse_last_admin_levels(&s))
        .transpose();
      // opt-out: only include_wkt=false switches it off; absent or any other value keeps it
      let include_wkt = query_param(&query_string, "include_wkt")
        .map(|s| s != "false")
        .unwrap_or(true);
      match (
        q,
        friendly_name_format,
        min_quality,
        bounding_wkt,
        last_admin_levels,
      ) {
        (None, _, _, _, _) => respond_json(
          request,
          StatusCode(400),
          r#"{"error":"missing query param: query"}"#,
        ),
        (_, Err(msg), _, _, _)
        | (_, _, Err(msg), _, _)
        | (_, _, _, Err(msg), _)
        | (_, _, _, _, Err(msg)) => {
          let body = serde_json::json!({ "error": msg }).to_string();
          respond_json(request, StatusCode(400), &body)
        }
        (
          Some(raw),
          Ok(friendly_name_format),
          Ok(min_quality),
          Ok(bounding),
          Ok(last_admin_levels),
        ) => {
          let input = crate::address::query_input::parse(&raw);
          if index.is_none() && input == crate::address::query_input::text {
            // degraded boot: text search needs the tantivy index; coordinate queries still work.
            respond_json(
              request,
              StatusCode(503),
              r#"{"error":"text_to_address service unavailable"}"#,
            )
          } else {
            let opts = crate::address::query_opts {
              friendly_name_format: friendly_name_format.as_deref(),
              min_quality,
              bounding,
              last_admin_levels,
              include_wkt,
            };
            let address = crate::address::address::open(conn, index, &preset.house_numbers);
            let result = match input {
              crate::address::query_input::coordinates {
                latitude,
                longitude,
              } => address.query_by_coordinates(latitude, longitude, &opts),
              crate::address::query_input::text => address.query_by_text(&raw, &opts),
            };
            match serde_json::to_string(&result) {
              Ok(body) => respond_json(request, StatusCode(200), &body),
              Err(_) => respond_json(
                request,
                StatusCode(500),
                r#"{"error":"serialization error"}"#,
              ),
            }
          }
        }
      }
    }
    _ => respond_json(request, StatusCode(404), r#"{"error":"not found"}"#),
  };

  log_access(&method, &url, status, start.elapsed().as_millis());
}

fn cors_origin() -> Header {
  Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap()
}

fn force_content_length<R: std::io::Read>(response: Response<R>) -> Response<R> {
  // tiny_http switches to chunked transfer-encoding (dropping content-length) for
  // bodies >= 32 kib; raising the threshold keeps every response on identity
  // encoding so content-length is always present for http/1.1 keep-alive clients.
  response.with_chunked_threshold(usize::MAX)
}

fn log_access(method: &Method, url: &str, status: u16, ms: u128) {
  let color = match status {
    200..=299 => "1;32",
    300..=399 => "1;36",
    400..=499 => "1;33",
    _ => "1;31",
  };
  println!("\x1b[{color}m{status}\x1b[0m {method} {url} in {ms}ms");
}

fn query_param(query_string: &str, key: &str) -> Option<String> {
  for pair in query_string.split('&') {
    if let Some((k, v)) = pair.split_once('=')
      && url_decode(k) == key
    {
      return Some(url_decode(v));
    }
  }
  None
}

fn hex_digit(b: u8) -> Option<u8> {
  match b {
    b'0'..=b'9' => Some(b - b'0'),
    b'a'..=b'f' => Some(b - b'a' + 10),
    b'A'..=b'F' => Some(b - b'A' + 10),
    _ => None,
  }
}

fn url_decode(s: &str) -> String {
  let bytes = s.as_bytes();
  let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
  let mut i = 0;
  while i < bytes.len() {
    match bytes[i] {
      b'%' if i + 2 < bytes.len() => match (hex_digit(bytes[i + 1]), hex_digit(bytes[i + 2])) {
        (Some(h), Some(l)) => {
          out.push((h << 4) | l);
          i += 3;
        }
        _ => {
          out.push(b'%');
          i += 1;
        }
      },
      b'+' => {
        out.push(b' ');
        i += 1;
      }
      b => {
        out.push(b);
        i += 1;
      }
    }
  }
  String::from_utf8_lossy(&out).into_owned()
}

fn parse_last_admin_levels(s: &str) -> Result<Vec<level>, String> {
  let parts: Vec<&str> = s.split(',').map(str::trim).collect();
  if parts.iter().all(|p| p.is_empty()) {
    return Err("last_admin_levels must be a comma-separated list of levels".to_string());
  }
  parts
    .iter()
    .map(|p| {
      level::parse(p).map_err(|error| match error {
        level_error::not_a_level_number(_) => format!("last_admin_levels: invalid level '{p}'"),
        level_error::outside_the_scale(value) => {
          format!("last_admin_levels: level {value} is not supported")
        }
      })
    })
    .collect()
}

fn respond_json(request: tiny_http::Request, status: StatusCode, body: &str) -> u16 {
  let code = status.0;
  let response = force_content_length(
    Response::from_string(body)
      .with_status_code(status)
      .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
      .with_header(cors_origin()),
  );
  request.respond(response).ok();
  code
}

fn respond_html(request: tiny_http::Request, status: StatusCode, body: &str) -> u16 {
  let code = status.0;
  let response = force_content_length(
    Response::from_string(body)
      .with_status_code(status)
      .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap())
      .with_header(Header::from_bytes("Cache-Control", "public, max-age=3600").unwrap())
      .with_header(cors_origin()),
  );
  request.respond(response).ok();
  code
}
