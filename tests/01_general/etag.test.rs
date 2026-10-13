use crate::common::ask::ask;
use crate::common::harness::{get, get_with, request, response, server};
use crate::general::world;

const ANY_POINT: &str = "-23.970949,-46.318730";
const ANY_TEXT: &str = "rua castro alves";
const AROUND_SANTOS: &str =
  "POLYGON((-46.4 -24.0, -46.2 -24.0, -46.2 -23.9, -46.4 -23.9, -46.4 -24.0))";

fn etag_of(r: &response) -> String {
  r.header("etag")
    .unwrap_or_else(|| panic!("a {} answer must carry an etag", r.status))
    .to_string()
}

fn md5_of(r: &response) -> String {
  format!("\"{:x}\"", md5::compute(&r.body))
}

// the status and the etag of one ask carrying `If-None-Match`
fn re_ask(s: &server, path: &str, if_none_match: &str) -> (u16, Option<String>) {
  let r = get_with(s.port, path, &[("If-None-Match", if_none_match)]);
  assert!(
    r.status != 304 || r.body.is_empty(),
    "a 304 must carry no body, got {} bytes",
    r.body.len()
  );
  (r.status, r.header("etag").map(str::to_string))
}

// 00.00. etag: a geocode answer carries a quoted md5 as its etag, the same for the same question
// and another for another
#[test]
#[ignore]
fn _00_00_a_geocode_answer_carries_an_etag_the_same_for_the_same_question() {
  let s = world().start_server();
  let path = ask(ANY_POINT).http_path();
  let first = get(s.port, &path);
  let again = get(s.port, &path);
  let other = get(s.port, &ask(ANY_TEXT).http_path());
  assert_eq!(first.status, 200);
  let etag = etag_of(&first);
  assert!(
    etag.len() == 34
      && etag.starts_with('"')
      && etag.ends_with('"')
      && etag[1..33].chars().all(|c| c.is_ascii_hexdigit()),
    "the etag must be a quoted md5, got {etag}"
  );
  assert_eq!(etag_of(&again), etag);
  assert_ne!(etag_of(&other), etag);
}

// 00.01. etag: `If-None-Match` with the etag of the answer is a 304 with no body, no content type
// and the etag again
#[test]
#[ignore]
fn _00_01_if_none_match_with_the_etag_answers_not_modified() {
  let s = world().start_server();
  let path = ask(ANY_POINT).http_path();
  let etag = etag_of(&get(s.port, &path));
  let r = get_with(s.port, &path, &[("If-None-Match", &etag)]);
  assert_eq!(r.status, 304);
  assert!(r.body.is_empty());
  assert_eq!(r.header("etag"), Some(etag.as_str()));
  assert_eq!(r.header("content-type"), None);
  assert_eq!(r.header("content-length"), Some("0"));
}

// 00.02. etag: another tag is a 200 again; a list, a weak tag and a star are a 304
#[test]
#[ignore]
fn _00_02_if_none_match_takes_a_list_a_weak_tag_and_a_star() {
  let s = world().start_server();
  let path = ask(ANY_POINT).http_path();
  let etag = etag_of(&get(s.port, &path));
  let not_modified = (304, Some(etag.clone()));
  assert_eq!(re_ask(&s, &path, "\"another\"").0, 200);
  assert_eq!(
    re_ask(&s, &path, &format!("\"another\", {etag}")),
    not_modified
  );
  assert_eq!(re_ask(&s, &path, &format!("W/{etag}")), not_modified);
  assert_eq!(re_ask(&s, &path, "*"), not_modified);
}

// 00.03. etag: every option is part of the question, so each variant has an etag of its own
#[test]
#[ignore]
fn _00_03_every_option_changes_the_etag() {
  let s = world().start_server();
  let variants = [
    ask(ANY_TEXT),
    ask(ANY_TEXT).with_wkt(),
    ask(ANY_TEXT).last_admin_levels("12"),
    ask(ANY_TEXT).friendly_name_format("{admin_level_8_name}"),
    ask(ANY_TEXT).min_quality("0.5"),
    ask(ANY_TEXT).bounding_wkt(AROUND_SANTOS),
  ];
  let mut etags: Vec<String> = variants
    .iter()
    .map(|variant| {
      let r = get(s.port, &variant.http_path());
      assert_eq!(r.status, 200, "{variant}");
      etag_of(&r)
    })
    .collect();
  etags.sort();
  etags.dedup();
  assert_eq!(
    etags.len(),
    variants.len(),
    "every variant must have an etag of its own"
  );
}

// 00.04. etag: the status report and the openapi document carry the md5 of their body and answer
// 304 too
#[test]
#[ignore]
fn _00_04_the_status_and_the_openapi_document_answer_not_modified_too() {
  let s = world().start_server();
  for path in ["/status", "/openapi.json"] {
    let r = get(s.port, path);
    assert_eq!(r.status, 200, "{path}");
    let etag = etag_of(&r);
    assert_eq!(etag, md5_of(&r), "{path}");
    assert_eq!(re_ask(&s, path, &etag), (304, Some(etag)), "{path}");
  }
}

// 00.05. etag: an error carries no etag and `If-None-Match` does not turn it into a 304
#[test]
#[ignore]
fn _00_05_an_error_carries_no_etag_and_ignores_if_none_match() {
  let s = world().start_server();
  let r = get(s.port, "/geocode");
  assert_eq!((r.status, r.header("etag")), (400, None));
  let r = get_with(s.port, "/geocode", &[("If-None-Match", "*")]);
  assert_eq!((r.status, r.header("etag")), (400, None));
  assert!(!r.body.is_empty(), "the error must keep its body");
}

// 00.06. etag: the preflight lets a browser send `If-None-Match` and the answer lets it read the etag
#[test]
#[ignore]
fn _00_06_cors_lets_a_browser_send_if_none_match_and_read_the_etag() {
  let s = world().start_server();
  let preflight = request(s.port, "OPTIONS", "/geocode");
  assert_eq!(
    preflight.header("access-control-allow-headers"),
    Some("Content-Type, If-None-Match")
  );
  let r = get(s.port, &ask(ANY_POINT).http_path());
  assert_eq!(r.header("access-control-expose-headers"), Some("ETag"));
}

// 00.07. etag: the openapi document names the etag header, the `If-None-Match` parameter and the
// 304 of a geocode answer
#[test]
#[ignore]
fn _00_07_the_openapi_spec_documents_the_etag() {
  let s = world().start_server();
  let spec = get(s.port, "/openapi.json").json();
  let geocode = &spec["paths"]["/geocode"]["get"];
  assert!(
    geocode["responses"]["200"]["headers"]["etag"].is_object(),
    "the 200 response must document etag: {}",
    geocode["responses"]["200"]
  );
  assert!(
    geocode["responses"]["304"].is_object(),
    "the 304 response must be documented: {}",
    geocode["responses"]
  );
  let names: Vec<&str> = geocode["parameters"]
    .as_array()
    .expect("geocode has parameters")
    .iter()
    .filter_map(|parameter| parameter["name"].as_str())
    .collect();
  assert!(
    names.contains(&"If-None-Match"),
    "If-None-Match must be a documented parameter: {names:?}"
  );
}

// 00.08. etag: the etag of a question is the same on another server, so a client keeps its
// answers across a restart; only an upgrade changes it
#[test]
#[ignore]
fn _00_08_the_etag_of_a_question_survives_a_restart() {
  let w = world();
  let path = ask(ANY_POINT).http_path();
  let etag_at = |s: server| etag_of(&get(s.port, &path));
  let first = etag_at(w.start_server());
  assert_eq!(etag_at(w.start_server()), first);
}
