use crate::common::ask::ask;
use crate::common::harness::{encode, get, plain, request, scenario, world, world_cell};
use crate::common::query::{first, matches};
use crate::extract::REGENERATE;
use crate::house_number::HOUSE_NUMBERS;
use serde_json::{Value, json};

pub static SCENARIO: scenario = scenario {
  region: "01_general",
  fixture_dir: "02_preset_brazil",
  pbf: "santos.osm.pbf",
  preset: "brazil",
};

pub(crate) static WORLD: world_cell = world_cell::new();

pub(crate) fn world() -> &'static world {
  WORLD.get(&SCENARIO)
}

const ANY_TEXT: &str = "rua";
const ANY_POINT: &str = "-23.970949,-46.318730";

// 00.00. pipeline integrity
#[test]
#[ignore]
fn _00_00_build_exits_zero() {
  let w = world();
  assert_eq!(
    w.build_status, 0,
    "build failed; stderr: {}",
    w.build_stderr
  );
}

// 00.01. pipeline integrity
#[test]
#[ignore]
fn _00_01_build_prints_every_stage_banner_in_order() {
  let w = world();
  let stdout = &w.build_stdout;
  let stages = [
    "── download",
    "── extract blob-chunks",
    "── extract header",
    "── extract osm-data",
    "── extract admin-levels",
    "── extract house-numbers",
    "── index",
    "── optimize",
  ];
  let mut previous = 0usize;
  for stage in stages {
    let at = stdout
      .find(stage)
      .unwrap_or_else(|| panic!("stage {stage:?} never ran:\n{stdout}"));
    assert!(at >= previous, "stage {stage:?} ran out of order");
    previous = at;
  }
}

// 00.02. pipeline integrity
#[test]
#[ignore]
fn _00_02_build_skips_the_download_stage_for_a_local_source() {
  let w = world();
  assert!(
    w.build_stdout.contains("skipping"),
    "a local source must skip the download stage:\n{}",
    w.build_stdout
  );
  assert!(
    !w.build_stderr
      .contains("is not a valid url nor a known geofabrik id"),
    "a local source must never be resolved against the geofabrik index:\n{}",
    w.build_stderr
  );
}

// 00.03. pipeline integrity
#[test]
#[ignore]
fn _00_03_sqlite_and_tantivy_artifacts_exist() {
  let w = world();
  let sqlite = std::fs::metadata(&w.sqlite_path).expect("the sqlite database must exist");
  assert!(sqlite.len() > 0, "the sqlite database must not be empty");
  assert!(
    w.index_path.join("meta.json").exists(),
    "the tantivy index must have been committed"
  );
}

// 00.04. pipeline integrity
#[test]
#[ignore]
fn _00_04_optimize_deletes_the_osm_data_sibling() {
  let w = world();
  assert!(
    !w.data_path.join("database.osm_data.sqlite3").exists(),
    "the intermediary osm_data database must be deleted by the optimize stage"
  );
}

// 00.05. pipeline integrity: optimize deletes every pbf under data_path; the copy lives beside it
#[test]
#[ignore]
fn _00_05_optimize_leaves_every_pbf_outside_data_path_alone() {
  let w = world();
  // optimize deletes every *.osm.pbf under data_path. the source copy is deliberately a
  // sibling of data_path rather than a child, so it survives — and so does the fixture.
  assert!(
    w.pbf.exists(),
    "the source copy lives outside data_path and must survive the optimize stage"
  );
  assert!(
    !w.data_path.join(w.scenario.pbf).exists(),
    "no pbf may be left inside data_path after optimize"
  );
  assert!(
    w.scenario.pbf_path().exists(),
    "the committed fixture must never be deleted by a test run"
  );
}

// 00.06. pipeline integrity
#[test]
#[ignore]
fn _00_06_extracted_admin_levels_cover_only_the_preset_levels() {
  let w = world();
  let conn = w.open_sqlite();
  let mut stmt = conn
    .prepare(
      "SELECT admin_level, COUNT(*) FROM admin_levels GROUP BY admin_level \
       ORDER BY admin_level",
    )
    .expect("failed to prepare");
  let rows: Vec<(u8, i64)> = stmt
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read a row"))
    .collect();

  let levels: Vec<u8> = rows.iter().map(|(l, _)| *l).collect();
  assert_eq!(
    levels,
    vec![2, 4, 8, 10, 12],
    "the preset decides which levels are extracted; the discarded ones are present in the \
     fixture and must have been filtered out. got {rows:?}"
  );
  for discarded in [3, 5, 6, 7, 9] {
    assert!(
      !levels.contains(&discarded),
      "level {discarded} is in the fixture and must not have been extracted"
    );
  }
}

// 00.07. pipeline integrity
#[test]
#[ignore]
fn _00_07_hierarchy_is_populated_for_every_admin_level() {
  let w = world();
  let conn = w.open_sqlite();
  let levels: i64 = conn
    .query_row("SELECT COUNT(*) FROM admin_levels", [], |r| r.get(0))
    .expect("failed to count admin_levels");
  let hierarchy: i64 = conn
    .query_row("SELECT COUNT(*) FROM admin_levels_hierarchy", [], |r| {
      r.get(0)
    })
    .expect("failed to count admin_levels_hierarchy");
  assert_eq!(
    levels, hierarchy,
    "every admin level must get a hierarchy row"
  );
}

// 00.08. pipeline integrity
#[test]
#[ignore]
fn _00_08_house_numbers_were_extracted_and_linked() {
  let w = world();
  let conn = w.open_sqlite();
  let total: i64 = conn
    .query_row("SELECT COUNT(*) FROM house_numbers", [], |r| r.get(0))
    .expect("failed to count house_numbers");
  assert!(total > 0, "the fixture carries addr:housenumber nodes");

  let orphans: i64 = conn
    .query_row(
      "SELECT COUNT(*) FROM house_numbers h \
       LEFT JOIN admin_levels a ON a.id = h.admin_level_id WHERE a.id IS NULL",
      [],
      |r| r.get(0),
    )
    .expect("failed to check the foreign key");
  assert_eq!(orphans, 0, "every house number must point at a street row");
}

// 00.09. pipeline integrity
#[test]
#[ignore]
fn _00_09_fixture_counts_stay_within_expected_bounds() {
  let w = world();
  let conn = w.open_sqlite();
  let streets: i64 = conn
    .query_row(
      "SELECT COUNT(*) FROM admin_levels WHERE admin_level = 12",
      [],
      |r| r.get(0),
    )
    .expect("failed to count streets");
  let (lo, hi) = (3_000, 40_000);
  assert!(
    (lo..hi).contains(&streets),
    "street count {streets} left its expected band {lo}..{hi}; regenerate deliberately"
  );

  let house_numbers: i64 = conn
    .query_row("SELECT COUNT(*) FROM house_numbers", [], |r| r.get(0))
    .expect("failed to count house numbers");
  let (lo, hi) = (100, 2_000);
  assert!(
    (lo..hi).contains(&house_numbers),
    "house number count {house_numbers} left its expected band {lo}..{hi}"
  );
}

// 00.10. pipeline integrity: the house-number count and the optimize steps, in order
#[test]
#[ignore]
fn _00_10_the_build_reports_the_house_numbers_it_linked_and_the_optimize_steps() {
  let w = world();
  let stdout = plain(&w.build_stdout);
  let house_numbers = format!("extracted {HOUSE_NUMBERS} house numbers in");
  let mut at = 0;
  for line in [
    house_numbers.as_str(),
    "── optimize",
    "deleted osm_data.sqlite3",
    "deleted 1 tables in",
    "optimizing sqlite file...",
    "optimized sqlite in",
    "after   main:",
  ] {
    let found = stdout[at..]
      .find(line)
      .unwrap_or_else(|| panic!("missing {line:?} after offset {at}; {REGENERATE}:\n{stdout}"));
    at += found + line.len();
  }
}

// 01.00. contract
#[test]
#[ignore]
fn _01_00_coordinates_query_prints_pretty_printed_json() {
  let w = world();
  let out = w.geolite(&["query", ANY_POINT, "--include-wkt", "false"]);
  assert_eq!(out.status, 0, "stderr: {}", out.stderr);
  serde_json::from_str::<Value>(&out.stdout).expect("stdout must be valid json");
  // to_string_pretty, not to_string: two-space indentation and `service` first, in
  // declaration order. re-rendering through a Value would sort the keys.
  assert!(
    out
      .stdout
      .starts_with("{\n  \"service\": \"coordinates_to_address\",\n  \"matches\": ["),
    "unexpected output shape: {}",
    &out.stdout[..out.stdout.len().min(120)]
  );
  assert!(
    out.stdout.contains("\n      \"friendly_name\": "),
    "nested keys must stay indented"
  );
}

// 01.01. contract
#[test]
#[ignore]
fn _01_01_coordinates_query_reports_the_coordinates_service() {
  assert_eq!(
    world().run(&[ANY_POINT])["service"],
    "coordinates_to_address"
  );
}

// 01.02. contract
#[test]
#[ignore]
fn _01_02_coordinate_matches_omit_house_number_and_relevance_signals() {
  let result = world().run(&[ANY_POINT]);
  let top = first(&result);
  assert!(
    top.get("house_number").is_none(),
    "the coordinate path never fills house_number; it appends a level 30 instead"
  );
  assert!(
    top["similarity"].is_null(),
    "similarity belongs to the text path"
  );
  assert!(top["score"].is_null(), "score belongs to the text path");
}

// 01.03. contract
#[test]
#[ignore]
fn _01_03_result_count_never_exceeds_max_results() {
  assert!(
    matches(&world().run(&[ANY_POINT])).len() <= 10,
    "MAX_RESULTS is 10"
  );
}

// 01.04. contract
#[test]
#[ignore]
fn _01_04_text_query_reports_the_text_service() {
  assert_eq!(world().run(&[ANY_TEXT])["service"], "text_to_address");
}

// 01.05. contract
#[test]
#[ignore]
fn _01_05_include_wkt_false_omits_every_geometry() {
  for m in matches(&world().run(&[ANY_TEXT])) {
    for level in m["admin_levels"]
      .as_array()
      .expect("admin_levels must be an array")
    {
      assert!(
        level.get("wkt").is_none(),
        "wkt must be skipped entirely when disabled"
      );
    }
  }
}

// 01.06. contract
#[test]
#[ignore]
fn _01_06_version_prints_the_crate_version() {
  let w = world();
  let out = w.geolite(&["--version"]);
  assert_eq!(out.status, 0);
  assert!(
    out.stdout.contains(env!("CARGO_PKG_VERSION")),
    "got {:?}",
    out.stdout
  );
}

// 01.07. contract
#[test]
#[ignore]
fn _01_07_help_lists_every_subcommand() {
  let w = world();
  let out = w.geolite(&["--help"]);
  assert_eq!(out.status, 0);
  for command in [
    "osm-pbf-file",
    "extract",
    "index",
    "optimize",
    "query",
    "http-server",
    "build",
    "merge",
  ] {
    assert!(
      out.stdout.contains(command),
      "--help must mention {command}"
    );
  }
}

// 01.08. contract
#[test]
#[ignore]
fn _01_08_root_serves_the_web_ui_as_html() {
  let w = world();
  let s = w.start_server();
  let r = get(s.port, "/");
  assert_eq!(r.status, 200);
  assert_eq!(r.header("content-type"), Some("text/html; charset=utf-8"));
  assert_eq!(r.header("cache-control"), Some("public, max-age=3600"));
  assert!(r.text().contains("<html"), "the embedded ui must be served");
}

// 01.09. contract
#[test]
#[ignore]
fn _01_09_docs_serves_the_swagger_ui() {
  let w = world();
  let s = w.start_server();
  let r = get(s.port, "/docs");
  assert_eq!(r.status, 200);
  assert!(
    r.text().contains("swagger"),
    "the docs route must serve swagger ui"
  );
}

// 01.10. contract
#[test]
#[ignore]
fn _01_10_status_reports_both_services_available() {
  let w = world();
  let s = w.start_server();
  let r = get(s.port, "/status");
  assert_eq!(r.status, 200);
  let body = r.json();
  assert_eq!(body["is_ok"], true);
  assert_eq!(body["databases"][0]["text_to_address"], true);
  assert_eq!(body["databases"][0]["coordinates_to_address"], true);
  assert!(
    body["databases"][0]["file"]
      .as_str()
      .expect("file must be a string")
      .ends_with("database.sqlite3"),
    "status must name the database it opened"
  );
}

// 01.11. contract
#[test]
#[ignore]
fn _01_11_an_unknown_route_returns_not_found() {
  let w = world();
  let s = w.start_server();
  let r = get(s.port, "/definitely-not-a-route");
  assert_eq!(r.status, 404);
  assert_eq!(r.text(), r#"{"error":"not found"}"#);
}

// 01.12. contract
#[test]
#[ignore]
fn _01_12_a_non_get_method_on_a_known_route_returns_method_not_allowed() {
  let w = world();
  let s = w.start_server();
  for (method, path) in [("POST", "/geocode"), ("PUT", "/status"), ("DELETE", "/")] {
    let r = request(s.port, method, path);
    assert_eq!(r.status, 405, "{method} {path}");
    assert_eq!(r.text(), r#"{"error":"method not allowed"}"#);
  }
}

// 01.13. contract
#[test]
#[ignore]
fn _01_13_a_non_get_method_on_an_unknown_route_returns_not_found() {
  let w = world();
  let s = w.start_server();
  assert_eq!(
    request(s.port, "POST", "/definitely-not-a-route").status,
    404
  );
}

// 01.14. contract
#[test]
#[ignore]
fn _01_14_options_on_a_known_route_returns_no_content_with_cors_preflight_headers() {
  let w = world();
  let s = w.start_server();
  let r = request(s.port, "OPTIONS", "/geocode");
  assert_eq!(r.status, 204);
  assert_eq!(
    r.header("access-control-allow-methods"),
    Some("GET, OPTIONS")
  );
  assert_eq!(
    r.header("access-control-allow-headers"),
    Some("Content-Type")
  );
  assert_eq!(r.header("content-length"), Some("0"));
  assert!(r.body.is_empty());
}

// 01.15. contract
#[test]
#[ignore]
fn _01_15_every_response_carries_the_cors_origin_header() {
  let w = world();
  let s = w.start_server();
  for path in [
    "/",
    "/docs",
    "/status",
    "/openapi.json",
    "/geocode",
    "/nope",
  ] {
    assert_eq!(
      get(s.port, path).header("access-control-allow-origin"),
      Some("*"),
      "{path} is missing the cors header"
    );
  }
}

// 01.16. contract
#[test]
#[ignore]
fn _01_16_every_response_declares_its_length_and_never_chunks() {
  let w = world();
  let s = w.start_server();
  for path in [
    "/",
    "/docs",
    "/status",
    "/openapi.json",
    "/geocode",
    "/nope",
  ] {
    let r = get(s.port, path);
    let declared: usize = r
      .header("content-length")
      .unwrap_or_else(|| panic!("{path} has no content-length"))
      .parse()
      .expect("content-length must be a number");
    assert_eq!(declared, r.body.len(), "{path} declared the wrong length");
    assert!(
      r.header("transfer-encoding").is_none(),
      "{path} must not chunk"
    );
  }
}

// 01.17. contract
#[test]
#[ignore]
fn _01_17_a_missing_query_param_returns_bad_request() {
  let w = world();
  let s = w.start_server();
  let r = get(s.port, "/geocode");
  assert_eq!(r.status, 400);
  assert_eq!(r.text(), r#"{"error":"missing query param: query"}"#);
}

// 01.18. contract
#[test]
#[ignore]
fn _01_18_only_the_literal_false_disables_the_geometry() {
  let w = world();
  let s = w.start_server();
  let base = format!("/geocode?query={}", encode(ANY_POINT));

  let disabled = get(s.port, &format!("{base}&include_wkt=false"));
  assert!(
    !disabled.text().contains("\"wkt\""),
    "include_wkt=false must drop the geometry"
  );

  for value in ["0", "no", "FALSE"] {
    let kept = get(s.port, &format!("{base}&include_wkt={value}"));
    assert!(
      kept.text().contains("\"wkt\""),
      "include_wkt={value} is not the literal `false` and must keep the geometry"
    );
  }
}

// 01.19. contract
#[test]
#[ignore]
fn _01_19_plus_and_percent_encoded_spaces_decode_identically() {
  let w = world();
  let s = w.start_server();
  let plus = get(s.port, &ask(ANY_TEXT).http_path().replace("%20", "+"));
  let percent = get(s.port, &ask(ANY_TEXT).http_path());
  assert_eq!(plus.status, 200);
  assert_eq!(
    plus.body, percent.body,
    "both encodings must decode to the same query"
  );
}

// 01.20. contract: the openapi skeleton — routes, methods, schema names — is exactly this
#[test]
#[ignore]
fn _01_20_the_openapi_spec_documents_exactly_these_routes_and_schemas() {
  let s = world().start_server();
  let r = get(s.port, "/openapi.json");
  assert_eq!(r.status, 200);
  let spec = r.json();

  assert_eq!(spec["openapi"], "3.1.0");
  assert_eq!(spec["info"]["title"], "geolite");
  assert_eq!(spec["info"]["version"], env!("CARGO_PKG_VERSION"));
  assert_eq!(spec["info"]["license"]["identifier"], "AGPL-3.0-only");

  let mut paths: Vec<&str> = spec["paths"]
    .as_object()
    .expect("paths must be an object")
    .keys()
    .map(String::as_str)
    .collect();
  paths.sort_unstable();
  assert_eq!(paths, ["/geocode", "/status"]);
  assert!(
    spec["paths"]["/geocode"]["get"].is_object(),
    "geocode is a GET"
  );
  assert!(
    spec["paths"]["/status"]["get"].is_object(),
    "status is a GET"
  );

  let mut schemas: Vec<&str> = spec["components"]["schemas"]
    .as_object()
    .expect("schemas must be an object")
    .keys()
    .map(String::as_str)
    .collect();
  schemas.sort_unstable();
  assert_eq!(
    schemas,
    [
      "ApiError",
      "admin_level",
      "database_status",
      "house_number_match",
      "query_house_number",
      "query_match",
      "query_match_attributes",
      "query_output",
      "query_service",
      "status_output",
    ]
  );
}

// 01.21. contract: every http error body, byte for byte
#[test]
#[ignore]
fn _01_21_the_error_bodies_are_exactly_these() {
  let s = world().start_server();
  let query = encode(ANY_TEXT);
  let cases: Vec<(&str, &str, String)> = vec![
    ("GET", "missing_query", "/geocode".to_string()),
    (
      "GET",
      "unknown_route",
      "/definitely-not-a-route".to_string(),
    ),
    ("POST", "method_not_allowed", "/geocode".to_string()),
    (
      "GET",
      "invalid_bounding_wkt",
      format!(
        "/geocode?query={query}&bounding_wkt={}",
        encode("POINT(1 1)")
      ),
    ),
    (
      "GET",
      "invalid_last_admin_levels",
      format!("/geocode?query={query}&last_admin_levels=abc"),
    ),
  ];

  let mut recorded = serde_json::Map::new();
  for (method, name, path) in cases {
    let r = request(s.port, method, &path);
    recorded.insert(
      name.to_string(),
      json!({ "status": r.status, "body": r.text() }),
    );
  }
  world().assert_exact(
    &Value::Object(recorded),
    &json!({
      "invalid_bounding_wkt": {
        "body": "{\"error\":\"bounding_wkt: must be a POLYGON or MULTIPOLYGON\"}",
        "status": 400,
      },
      "invalid_last_admin_levels": {
        "body": "{\"error\":\"last_admin_levels: invalid level 'abc'\"}",
        "status": 400,
      },
      "method_not_allowed": {
        "body": "{\"error\":\"method not allowed\"}",
        "status": 405,
      },
      "missing_query": {
        "body": "{\"error\":\"missing query param: query\"}",
        "status": 400,
      },
      "unknown_route": {
        "body": "{\"error\":\"not found\"}",
        "status": 404,
      },
    }),
  );
}

// 01.22. contract: the cli and the http api read a coordinate with one rule; everything else is text
#[test]
#[ignore]
fn _01_22_the_input_is_a_coordinate_only_when_it_parses_as_lat_lon() {
  let w = world();
  let s = w.start_server();
  for (input, service) in [
    (" -23.970949 , -46.318730 ", "coordinates_to_address"),
    ("0,0", "coordinates_to_address"),
    ("91.0,0.0", "text_to_address"),
    ("-23.970949", "text_to_address"),
    ("1,2,3", "text_to_address"),
  ] {
    w.assert_both(&s, &ask(input), &json!({ "service": service }));
  }
}

// 02.00. dead case
#[test]
#[ignore]
fn _02_00_a_nonsense_query_returns_no_matches() {
  let w = world();
  let out = w.geolite(&["query", "zzzz qqqq wwww", "--include-wkt", "false"]);
  assert_eq!(out.status, 0, "an empty result is not an error");
  let result: Value = serde_json::from_str(&out.stdout).expect("stdout must be json");
  assert!(matches(&result).is_empty());
}

// 02.01. dead case
#[test]
#[ignore]
fn _02_01_build_with_a_missing_source_exits_one() {
  let w = world();
  let out = w.geolite(&["build", "./does-not-exist.osm.pbf"]);
  assert_eq!(out.status, 1);
  assert!(
    out.stderr.contains("source file not found"),
    "stderr: {}",
    out.stderr
  );
}

// 02.02. dead case
#[test]
#[ignore]
fn _02_02_query_without_a_sqlite_exits_one() {
  let w = world();
  let missing = w.root.join("absent.sqlite3");
  let out = w.geolite(&["--sqlite-path", &missing.to_string_lossy(), "query", "x"]);
  assert_eq!(out.status, 1);
  assert!(
    out.stderr.contains("sqlite not found"),
    "stderr: {}",
    out.stderr
  );
}

// 02.03. dead case: the cli loads the index before it reads the input, so a coordinate is refused
// too, where the http api keeps answering coordinates (03.02)
#[test]
#[ignore]
fn _02_03_query_without_a_tantivy_index_exits_one_for_text_and_for_coordinates() {
  let w = world();
  let missing = w.root.join("absent.tantivy");
  for input in [ANY_TEXT, ANY_POINT] {
    let out = w.geolite(&["--index-path", &missing.to_string_lossy(), "query", input]);
    assert_eq!(out.status, 1, "input {input:?}");
    assert!(
      out.stderr.contains("tantivy index not found"),
      "input {input:?}, stderr: {}",
      out.stderr
    );
  }
}

// 02.04. dead case
#[test]
#[ignore]
fn _02_04_an_out_of_range_min_quality_is_rejected_by_clap() {
  let w = world();
  let out = w.geolite(&["query", ANY_TEXT, "--min-quality", "2"]);
  assert_eq!(out.status, 2);
  assert!(
    out.stderr.contains("between 0.0 and 1.0"),
    "stderr: {}",
    out.stderr
  );
}

// 02.05. dead case
#[test]
#[ignore]
fn _02_05_a_non_area_bounding_wkt_is_rejected_by_clap() {
  let w = world();
  let out = w.geolite(&["query", ANY_TEXT, "--bounding-wkt", "POINT(1 1)"]);
  assert_eq!(out.status, 2);
  assert!(
    out.stderr.to_lowercase().contains("polygon"),
    "stderr: {}",
    out.stderr
  );
}

// 02.06. dead case
#[test]
#[ignore]
fn _02_06_an_unknown_friendly_name_placeholder_is_rejected_by_clap() {
  let w = world();
  let out = w.geolite(&["query", ANY_TEXT, "--friendly-name-format", "{foo}"]);
  assert_eq!(out.status, 2);
  assert!(out.stderr.contains("foo"), "stderr: {}", out.stderr);
}

// 02.07. dead case
#[test]
#[ignore]
fn _02_07_an_unknown_subcommand_exits_two() {
  let w = world();
  assert_eq!(w.geolite(&["definitely-not-a-command"]).status, 2);
}

// 02.08. dead case
#[test]
#[ignore]
fn _02_08_an_unknown_friendly_name_placeholder_returns_bad_request() {
  let w = world();
  let s = w.start_server();
  let path = format!(
    "/geocode?query={}&friendly_name_format={}",
    encode(ANY_TEXT),
    encode("{foo}")
  );
  let r = get(s.port, &path);
  assert_eq!(r.status, 400);
  assert!(r.text().contains("foo"), "body: {}", r.text());
}

// 02.09. dead case
#[test]
#[ignore]
fn _02_09_a_non_area_bounding_wkt_returns_bad_request() {
  let w = world();
  let s = w.start_server();
  let path = format!(
    "/geocode?query={}&bounding_wkt={}",
    encode(ANY_TEXT),
    encode("POINT(1 1)")
  );
  assert_eq!(get(s.port, &path).status, 400);
}

// 02.10. dead case
#[test]
#[ignore]
fn _02_10_an_invalid_last_admin_levels_returns_bad_request() {
  let w = world();
  let s = w.start_server();
  let path = format!("/geocode?query={}&last_admin_levels=abc", encode(ANY_TEXT));
  assert_eq!(get(s.port, &path).status, 400);
}

// 02.11. dead case: http drops an out-of-range quality where the cli rejects it
#[test]
#[ignore]
fn _02_11_an_out_of_range_quality_is_ignored_rather_than_rejected() {
  let w = world();
  // a deliberate asymmetry with the cli, which errors in parse_min_quality. the http handler
  // drops the value with .filter(...) instead.
  let s = w.start_server();
  let path = format!("{}&quality=5", ask(ANY_TEXT).http_path());
  let r = get(s.port, &path);
  assert_eq!(r.status, 200, "body: {}", r.text());
  assert!(!r.json()["matches"].as_array().expect("matches").is_empty());
}

// 02.12. dead case
#[test]
#[ignore]
fn _02_12_a_level_outside_the_scale_in_last_admin_levels_returns_bad_request() {
  let w = world();
  let s = w.start_server();
  let path = format!("/geocode?query={}&last_admin_levels=8,11", encode(ANY_TEXT));
  let r = get(s.port, &path);
  assert_eq!(r.status, 400, "body: {}", r.text());
  assert_eq!(
    r.json()["error"],
    "last_admin_levels: level 11 is not supported"
  );
}

// 02.13. dead case
#[test]
#[ignore]
fn _02_13_a_level_outside_the_scale_in_last_admin_levels_is_rejected_by_clap() {
  let w = world();
  let out = w.geolite(&["query", ANY_TEXT, "--last-admin-levels", "8,11"]);
  assert_eq!(out.status, 2);
  assert!(
    out.stderr.contains("level 11 is not supported"),
    "stderr: {}",
    out.stderr
  );
}

// 03.00. degraded mode
#[test]
#[ignore]
fn _03_00_status_reports_service_unavailable_without_an_index() {
  let w = world();
  let s = w.start_degraded_server();
  let r = get(s.port, "/status");
  assert_eq!(r.status, 503);
  let body = r.json();
  assert_eq!(body["is_ok"], false);
  assert_eq!(body["databases"][0]["text_to_address"], false);
  assert_eq!(body["databases"][0]["coordinates_to_address"], true);
}

// 03.01. degraded mode
#[test]
#[ignore]
fn _03_01_a_text_query_is_unavailable_without_an_index() {
  let w = world();
  let s = w.start_degraded_server();
  let r = get(s.port, &ask(ANY_TEXT).http_path());
  assert_eq!(r.status, 503);
  assert_eq!(
    r.text(),
    r#"{"error":"text_to_address service unavailable"}"#
  );
}

// 03.02. degraded mode
#[test]
#[ignore]
fn _03_02_a_coordinate_query_still_works_without_an_index() {
  let w = world();
  let s = w.start_degraded_server();
  let r = get(s.port, &ask(ANY_POINT).http_path());
  assert_eq!(r.status, 200);
  let body = r.json();
  assert_eq!(body["service"], "coordinates_to_address");
  assert!(!body["matches"].as_array().expect("matches").is_empty());
}

// 03.03. degraded mode
#[test]
#[ignore]
fn _03_03_the_static_routes_still_serve_without_an_index() {
  let w = world();
  let s = w.start_degraded_server();
  for path in ["/", "/docs", "/openapi.json"] {
    assert_eq!(get(s.port, path).status, 200, "{path} must still serve");
  }
}

// 03.04. degraded mode
#[test]
#[ignore]
fn _03_04_a_degraded_boot_warns_on_stderr() {
  let w = world();
  let s = w.start_degraded_server();
  let _ = get(s.port, "/status"); // make sure the warning is flushed before reading
  assert!(
    s.stderr().contains("tantivy index not found at"),
    "expected a warning, got: {}",
    s.stderr()
  );
}

// 03.05. degraded mode: the 503 follows the same reading of the input as the dispatch
#[test]
#[ignore]
fn _03_05_an_out_of_range_coordinate_is_a_text_query_and_is_unavailable_without_an_index() {
  let w = world();
  let s = w.start_degraded_server();
  for (input, status) in [("91.0,0.0", 503), (" -23.970949 , -46.318730 ", 200)] {
    assert_eq!(
      get(s.port, &ask(input).http_path()).status,
      status,
      "input {input:?}"
    );
  }
}

// 04.00. precision guarantee: streets have no distance cap; only --min-quality removes a far match
#[test]
#[ignore]
fn _04_00_distant_coordinates_still_resolve_because_streets_have_no_distance_cap() {
  let far = world().run(&["-25.5,-45.0"]);
  let distance = first(&far)["coordinates_distance_in_meters"]
    .as_u64()
    .expect("a coordinate match must report a distance");
  let floor = 10_000;
  assert!(
    distance > floor,
    "expected a very distant way, got {distance} m"
  );

  let filtered = world().run(&["-25.5,-45.0", "--min-quality", "0.5"]);
  assert!(
    matches(&filtered).is_empty(),
    "a quality floor must drop matches that far away"
  );
}

// 05.00. regression guard: id is admin_level_id::from_way(osm_id), a pure function, never an insert-order rowid
#[test]
#[ignore]
fn _05_00_match_id_is_the_packed_osm_id() {
  let result = world().run(&[ANY_POINT]);
  for m in matches(&result) {
    let id = m["id"].as_u64().expect("id must be a number");
    let leaf = m["admin_levels"]
      .as_array()
      .and_then(|a| a.iter().rev().find(|l| l["level"].as_u64() != Some(30)))
      .expect("a match must carry at least one admin level");
    match (
      leaf["osm_way_id"].as_u64(),
      leaf["osm_relation_id"].as_u64(),
    ) {
      (Some(way), _) => assert_eq!(id, way * 2, "way ids pack as osm_id << 1"),
      (None, Some(rel)) => assert_eq!(id, rel * 2 + 1, "relation ids set the low bit"),
      (None, None) => panic!("the leaf admin level carries neither a way nor a relation id"),
    }
  }
}
