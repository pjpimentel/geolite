use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tiny_http::{Header, Method, Response, Server, StatusCode};

use crate::admin_level::{level, level_error};
use crate::admin_level_hierarchy::tantivy_index;
use crate::database::store::base_files;

mod etag;
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

pub struct served_base {
  files: base_files,
  index: Option<Arc<tantivy_index>>,
}

struct worker_base<'a> {
  files: &'a base_files,
  conn: rusqlite::Connection,
  index: Option<&'a tantivy_index>,
}

pub fn serve(bound: bound_server, bases: Vec<base_files>, threads: u8) {
  let addr = bound.addr();
  let server = bound.server;

  // degraded boot: a base without its index answers coordinates only (reported via /status)
  // instead of refusing to start
  let boosts = crate::presets::DEFAULT.index_addresses.boosts;
  let served: Vec<served_base> = bases
    .into_iter()
    .map(|files| {
      let index = crate::admin_level_hierarchy::search_index::load(Path::new(&files.index), boosts)
        .map(Arc::new);
      if index.is_none() {
        eprintln!(
          "\x1b[1;33mwarn\x1b[0m: tantivy index not found at {} — text_to_address disabled for {}; run `geolite exec index-addresses {}` to enable",
          files.index,
          files.file_name(),
          files.iso
        );
      }
      served_base { files, index }
    })
    .collect();
  let served = Arc::new(served);

  // SAFETY: on_shutdown_signal only performs an atomic store, which is async-signal-safe.
  unsafe {
    signal(SIGINT, on_shutdown_signal);
    signal(SIGTERM, on_shutdown_signal);
  }

  // the e2e harness reads this line to learn the port a `--port 0` server took: keep its shape
  println!(
    "\x1b[1;32mlistening\x1b[0m on {addr}  bases: {}",
    served
      .iter()
      .map(|base| base.files.sqlite.as_str())
      .collect::<Vec<_>>()
      .join(", ")
  );

  let handles: Vec<_> = (0..threads)
    .map(|_| {
      let server = server.clone();
      let served = served.clone();
      std::thread::spawn(move || {
        let bases: Vec<worker_base> = served
          .iter()
          .map(|base| worker_base {
            files: &base.files,
            conn: crate::database::store::open_base_immutable(&base.files.sqlite),
            index: base.index.as_deref(),
          })
          .collect();
        // recv_timeout instead of the blocking incoming_requests() so each worker rechecks the
        // shutdown flag every poll interval (tiny_http's unblock() only wakes one thread).
        while !SHUTDOWN.load(Ordering::Relaxed) {
          match server.recv_timeout(Duration::from_millis(250)) {
            Ok(Some(request)) => handle(request, &bases),
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

fn parse_countries(raw: &str) -> Result<Vec<String>, String> {
  let isos: Vec<String> = raw
    .split(',')
    .map(str::trim)
    .filter(|iso| !iso.is_empty())
    .map(str::to_string)
    .collect();
  if isos.is_empty() {
    return Err("country must be a comma-separated list of iso 3166-1 alpha-2 codes".to_string());
  }
  Ok(isos)
}

fn select<'a>(bases: &'a [worker_base<'a>], countries: Option<&[String]>) -> Result<Vec<&'a worker_base<'a>>, String> {
  let Some(countries) = countries else {
    return Ok(bases.iter().collect());
  };
  countries
    .iter()
    .map(|iso| {
      bases
        .iter()
        .find(|base| base.files.iso.eq_ignore_ascii_case(iso))
        .ok_or_else(|| format!("country: no base for '{iso}'"))
    })
    .collect()
}

fn handle(request: tiny_http::Request, bases: &[worker_base]) {
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
        .with_header(cors_allowed_headers()),
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
      let report = status::build(
        bases
          .iter()
          .map(|base| (base.files.file_name(), &base.conn, base.index)),
      );
      match serde_json::to_string(&report) {
        Ok(body) if report.is_ok => respond_ok(request, &body),
        Ok(body) => respond_json(request, StatusCode(503), &body),
        Err(_) => respond_json(
          request,
          StatusCode(500),
          r#"{"error":"serialization error"}"#,
        ),
      }
    }
    "/openapi.json" => respond_ok(request, openapi::spec_json()),
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
      let countries = query_param(&query_string, "country")
        .map(|s| parse_countries(&s))
        .transpose()
        .and_then(|countries| select(bases, countries.as_deref()).map(|selected| (countries, selected)));
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
        countries,
      ) {
        (None, _, _, _, _, _) => respond_json(
          request,
          StatusCode(400),
          r#"{"error":"missing query param: query"}"#,
        ),
        (_, Err(msg), _, _, _, _)
        | (_, _, Err(msg), _, _, _)
        | (_, _, _, Err(msg), _, _)
        | (_, _, _, _, Err(msg), _)
        | (_, _, _, _, _, Err(msg)) => {
          let body = serde_json::json!({ "error": msg }).to_string();
          respond_json(request, StatusCode(400), &body)
        }
        (
          Some(raw),
          Ok(friendly_name_format),
          Ok(min_quality),
          Ok(bounding),
          Ok(last_admin_levels),
          Ok((countries, selected)),
        ) => {
          let input = crate::address::query_input::parse(&raw);
          let no_index = selected.iter().all(|base| base.index.is_none());
          if no_index && input == crate::address::query_input::text {
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
            let etag = etag::of_question(&raw, &opts, countries.as_deref());
            if etag::none_match(&request, &etag) {
              respond_not_modified(request, &etag)
            } else {
              let asked: Vec<crate::address::base> = selected
                .iter()
                .map(|base| crate::address::base {
                  conn: &base.conn,
                  index: base.index,
                })
                .collect();
              let result = match input {
                crate::address::query_input::coordinates {
                  latitude,
                  longitude,
                } => crate::address::query_by_coordinates_across(&asked, latitude, longitude, &opts),
                crate::address::query_input::text => {
                  crate::address::query_by_text_across(&asked, &raw, &opts)
                }
              };
              match serde_json::to_string(&result) {
                Ok(body) => respond_ok_with_etag(request, &body, &etag),
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
    }
    _ => respond_json(request, StatusCode(404), r#"{"error":"not found"}"#),
  };

  log_access(&method, &url, status, start.elapsed().as_millis());
}

fn cors_origin() -> Header {
  Header::from_bytes("Access-Control-Allow-Origin", "*").unwrap()
}

fn cors_allowed_headers() -> Header {
  Header::from_bytes("Access-Control-Allow-Headers", "Content-Type, If-None-Match").unwrap()
}

fn cors_exposed_headers() -> Header {
  Header::from_bytes("Access-Control-Expose-Headers", "ETag").unwrap()
}

fn json_content_type() -> Header {
  Header::from_bytes("Content-Type", "application/json").unwrap()
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
      .with_header(json_content_type())
      .with_header(cors_origin()),
  );
  request.respond(response).ok();
  code
}

fn respond_ok(request: tiny_http::Request, body: &str) -> u16 {
  let etag = etag::of_body(body);
  if etag::none_match(&request, &etag) {
    respond_not_modified(request, &etag)
  } else {
    respond_ok_with_etag(request, body, &etag)
  }
}

fn respond_ok_with_etag(request: tiny_http::Request, body: &str, etag: &str) -> u16 {
  let response = force_content_length(
    Response::from_string(body)
      .with_status_code(StatusCode(200))
      .with_header(json_content_type())
      .with_header(etag::header(etag))
      .with_header(cors_origin())
      .with_header(cors_exposed_headers()),
  );
  request.respond(response).ok();
  200
}

fn respond_not_modified(request: tiny_http::Request, etag: &str) -> u16 {
  // from_data, not from_string(""): the latter adds a text/plain content type a 304 must not carry
  let response = force_content_length(
    Response::from_data(Vec::new())
      .with_status_code(StatusCode(304))
      .with_header(etag::header(etag))
      .with_header(cors_origin())
      .with_header(cors_exposed_headers()),
  );
  request.respond(response).ok();
  304
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
