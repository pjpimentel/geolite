use serde::Serialize;
use tiny_http::{Header, Request};

use crate::address::query_opts;

#[derive(Serialize)]
struct question<'a> {
  version: &'static str,
  input: &'a str,
  friendly_name_format: Option<&'a str>,
  min_quality: Option<f64>,
  bounding_wkt: Option<&'a str>,
  last_admin_levels: Option<Vec<u8>>,
  include_wkt: bool,
  country: Option<Vec<String>>,
}

pub fn of_question(input: &str, opts: &query_opts<'_>, country: Option<&[String]>) -> String {
  let query_opts {
    friendly_name_format,
    min_quality,
    bounding,
    last_admin_levels,
    include_wkt,
  } = opts;
  let question = question {
    version: env!("CARGO_PKG_VERSION"),
    input,
    friendly_name_format: *friendly_name_format,
    min_quality: *min_quality,
    bounding_wkt: bounding.as_ref().map(|bounding| bounding.wkt.as_str()),
    last_admin_levels: last_admin_levels
      .as_ref()
      .map(|levels| levels.iter().map(|level| level.value()).collect()),
    include_wkt: *include_wkt,
    country: country.map(|isos| isos.iter().map(|iso| iso.to_ascii_lowercase()).collect()),
  };
  of_body(&serde_json::to_string(&question).expect("a question serializes"))
}

pub fn of_body(body: &str) -> String {
  format!("\"{:x}\"", md5::compute(body))
}

pub fn header(etag: &str) -> Header {
  Header::from_bytes("ETag", etag).unwrap()
}

pub fn none_match(request: &Request, etag: &str) -> bool {
  request
    .headers()
    .iter()
    .filter(|header| header.field.equiv("If-None-Match"))
    .flat_map(|header| header.value.as_str().split(','))
    .map(str::trim)
    .any(|tag| tag == "*" || opaque(tag) == etag)
}

fn opaque(tag: &str) -> &str {
  tag.strip_prefix("W/").unwrap_or(tag)
}
