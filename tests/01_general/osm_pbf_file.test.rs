use std::path::Path;

use crate::common::harness::{open_sqlite_at, output, world};
use crate::common::stub;
use crate::general::world;

const INDEX: &str = r#"{"features":[
  {"properties":{"id":"alpha","name":"Alpha","urls":{"pbf":"{base}/alpha.osm.pbf"}}},
  {"properties":{"id":"beta","name":"Beta"}}
]}"#;

const INDEX_WITH_GAMMA: &str = r#"{"features":[
  {"properties":{"id":"alpha","name":"Alpha","urls":{"pbf":"{base}/alpha.osm.pbf"}}},
  {"properties":{"id":"beta","name":"Beta"}},
  {"properties":{"id":"gamma","name":"Gamma","urls":{"pbf":"{base}/gamma.osm.pbf"}}}
]}"#;

const INDEX_WITH_COVERAGE: &str = r#"{"features":[
  {"properties":{"id":"alpha","name":"Alpha","urls":{"pbf":"{base}/alpha.osm.pbf"}},"geometry":{"type":"MultiPolygon","coordinates":[[[[1,42],[2,42],[2,43],[1,43],[1,42]]]]}},
  {"properties":{"id":"beta","name":"Beta"}}
]}"#;

struct row {
  id: i64,
  origin: u8,
  origin_id: Option<String>,
  origin_name: Option<String>,
  url: Option<String>,
  path: Option<String>,
}

fn ledger(conn: &rusqlite::Connection) -> Vec<row> {
  conn
    .prepare("SELECT id, origin, origin_id, origin_name, url, path FROM osm_pbf_files ORDER BY id")
    .expect("failed to prepare the ledger query")
    .query_map([], |r| {
      Ok(row {
        id: r.get(0)?,
        origin: r.get(1)?,
        origin_id: r.get(2)?,
        origin_name: r.get(3)?,
        url: r.get(4)?,
        path: r.get(5)?,
      })
    })
    .expect("failed to read the ledger")
    .collect::<Result<Vec<_>, _>>()
    .expect("failed to collect the ledger")
}

fn count(conn: &rusqlite::Connection, sql: &str) -> i64 {
  conn
    .query_row(sql, [], |r| r.get(0))
    .expect("failed to count")
}

fn index_exists(conn: &rusqlite::Connection, name: &str) -> bool {
  count(
    conn,
    &format!("SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = '{name}'"),
  ) == 1
}

fn fixture_bytes(w: &world) -> Vec<u8> {
  std::fs::read(&w.pbf).expect("failed to read the fixture")
}

fn database(dir: &Path) -> rusqlite::Connection {
  open_sqlite_at(&dir.join("database.sqlite3"))
}

fn joined(dir: &Path, name: &str) -> String {
  dir.join(name).to_string_lossy().into_owned()
}

fn coverage(conn: &rusqlite::Connection, origin_id: &str) -> Option<String> {
  conn
    .query_row(
      "SELECT origin_wkt FROM osm_pbf_files WHERE origin_id = ?1",
      [origin_id],
      |r| r.get::<_, Option<Vec<u8>>>(0),
    )
    .expect("failed to read the coverage")
    .map(|bytes| String::from_utf8(bytes).expect("the coverage is utf-8"))
}

fn ls(w: &world, dir: &Path, endpoint: &str) -> output {
  let out = w.geolite_in(
    dir,
    &["osm-pbf-file", "--ls-endpoint", endpoint, "ls", "geofabrik"],
  );
  assert_eq!(out.status, 0, "ls failed:\n{}", out.stderr);
  out
}

fn download(w: &world, dir: &Path, endpoint: &str, input: &str) {
  let out = w.geolite_in(
    dir,
    &[
      "--threads",
      "1",
      "osm-pbf-file",
      "--ls-endpoint",
      endpoint,
      "download",
      input,
    ],
  );
  assert_eq!(out.status, 0, "download {input} failed:\n{}", out.stderr);
}

// 00. the ledger of the shared build
#[test]
#[ignore]
fn _00_the_ledger_row_of_a_local_build_has_a_local_path_origin() {
  let w = world();
  let conn = w.open_sqlite();
  let rows = ledger(&conn);
  assert_eq!(
    rows.len(),
    1,
    "a single-file build writes a single ledger row"
  );
  let file = &rows[0];
  assert_eq!(file.origin, 0);
  assert_eq!(file.origin_id, None);
  assert_eq!(file.origin_name.as_deref(), Some("santos.osm.pbf"));
  assert_eq!(file.url, None);
  assert_eq!(file.path.as_deref(), Some(w.pbf.to_string_lossy().as_ref()));

  let (program, nodes, ways, relations, houses): (Option<String>, i64, i64, i64, i64) = conn
    .query_row(
      "SELECT osm_header_writingprogram, node_count, way_count, relation_count, house_numbers_count FROM osm_pbf_files",
      [],
      |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )
    .expect("failed to read the ledger columns");
  assert!(
    program.is_some(),
    "the header stage must fill osm_header_writingprogram"
  );
  assert!(
    nodes > 0 && ways > 0 && relations > 0,
    "the osm-data stage must fill the counts"
  );
  assert_eq!(houses, count(&conn, "SELECT COUNT(*) FROM house_numbers"));
  assert_eq!(
    count(&conn, "SELECT admin_levels_count FROM osm_pbf_files"),
    count(
      &conn,
      "SELECT COUNT(*) FROM admin_levels WHERE wkb IS NOT NULL"
    )
  );
  assert_eq!(count(&conn, "PRAGMA user_version"), 2);
}

// 01. schema version
#[test]
#[ignore]
fn _01_a_write_command_refuses_a_database_stamped_with_another_schema_version() {
  let w = world();
  let dir = w.scratch("foreign_version_write");
  std::fs::copy(&w.sqlite_path, dir.join("database.sqlite3")).expect("failed to copy the database");
  {
    let conn = rusqlite::Connection::open(dir.join("database.sqlite3")).expect("failed to open");
    conn
      .pragma_update(None, "user_version", 1)
      .expect("failed to stamp user_version");
  }
  let out = w.geolite_in(
    &dir,
    &["extract", "osm-pbf-header", &w.pbf.to_string_lossy()],
  );
  assert_ne!(
    out.status, 0,
    "a write command must refuse the database:\n{}",
    out.stdout
  );
  assert!(
    out.stderr.contains("incompatible schema version")
      && out.stderr.contains("found 1, expected 2"),
    "stderr: {}",
    out.stderr
  );
}

// 02. schema version
#[test]
#[ignore]
fn _02_query_still_answers_on_a_database_stamped_with_another_schema_version() {
  let w = world();
  let dir = w.scratch("foreign_version_query");
  std::fs::copy(&w.sqlite_path, dir.join("database.sqlite3")).expect("failed to copy the database");
  {
    let conn = rusqlite::Connection::open(dir.join("database.sqlite3")).expect("failed to open");
    conn
      .pragma_update(None, "user_version", 1)
      .expect("failed to stamp user_version");
  }
  let out = w.geolite_in(
    &dir,
    &[
      "--index-path",
      &w.index_path.to_string_lossy(),
      "query",
      "rua",
      "--include-wkt",
      "false",
    ],
  );
  assert_eq!(
    out.status, 0,
    "query must ignore the schema version:\n{}",
    out.stderr
  );
  let json: serde_json::Value =
    serde_json::from_str(&out.stdout).expect("query printed invalid json");
  assert!(
    !json["matches"].as_array().expect("matches").is_empty(),
    "the stamped copy still answers"
  );
}

// 03. ls local
#[test]
#[ignore]
fn _03_ls_local_lists_the_pbf_files_without_creating_a_database() {
  let w = world();
  let dir = w.scratch("ls_local");
  std::fs::copy(&w.pbf, dir.join("santos.osm.pbf")).expect("failed to copy the fixture");
  let size = std::fs::metadata(&w.pbf).expect("fixture metadata").len();

  let out = w.geolite_in(&dir, &["osm-pbf-file", "ls", "local"]);
  assert_eq!(out.status, 0, "stderr: {}", out.stderr);
  assert!(
    out.stdout.contains("santos.osm.pbf"),
    "stdout: {}",
    out.stdout
  );
  assert!(
    out.stdout.contains(&format!("{size} bytes")),
    "stdout: {}",
    out.stdout
  );

  let mut left: Vec<String> = std::fs::read_dir(&dir)
    .expect("failed to list the scratch dir")
    .flatten()
    .map(|e| e.file_name().to_string_lossy().into_owned())
    .collect();
  left.sort();
  assert_eq!(
    left,
    vec!["santos.osm.pbf".to_string()],
    "ls local must not create a database"
  );
}

// 04. ls geofabrik
#[test]
#[ignore]
fn _04_ls_geofabrik_caches_the_index_and_answers_from_the_cache_when_the_endpoint_is_down() {
  let w = world();
  let dir = w.scratch("ls_geofabrik_cache");
  let mut s = stub::start(INDEX, vec![]);
  let endpoint = s.url("/index.json");

  let out = w.geolite_in(
    &dir,
    &[
      "osm-pbf-file",
      "--ls-endpoint",
      &endpoint,
      "ls",
      "geofabrik",
    ],
  );
  assert_eq!(out.status, 0, "stderr: {}", out.stderr);
  assert!(
    out.stdout.contains("alpha") && out.stdout.contains("beta"),
    "stdout: {}",
    out.stdout
  );
  let rows = ledger(&database(&dir));
  assert_eq!(rows.len(), 2);
  let alpha = rows
    .iter()
    .find(|r| r.origin_id.as_deref() == Some("alpha"))
    .expect("alpha row");
  let beta = rows
    .iter()
    .find(|r| r.origin_id.as_deref() == Some("beta"))
    .expect("beta row");
  assert_eq!(
    (alpha.origin, alpha.origin_name.as_deref()),
    (1, Some("Alpha"))
  );
  assert_eq!(alpha.url.as_deref(), Some(s.url("/alpha.osm.pbf").as_str()));
  assert_eq!((beta.origin, beta.url.as_deref()), (1, Some("-")));

  s.stop();
  let cached = w.geolite_in(
    &dir,
    &[
      "osm-pbf-file",
      "--ls-endpoint",
      "http://127.0.0.1:1/unused.json",
      "ls",
      "geofabrik",
    ],
  );
  assert_eq!(
    cached.status, 0,
    "the cache must answer without the endpoint:\n{}",
    cached.stderr
  );
  assert_eq!(cached.stdout, out.stdout);
}

// 05. ls geofabrik
#[test]
#[ignore]
fn _05_ls_geofabrik_with_recreate_cache_refetches_the_index() {
  let w = world();
  let dir = w.scratch("ls_geofabrik_recreate");
  let first = stub::start(INDEX, vec![]);
  let out = w.geolite_in(
    &dir,
    &[
      "osm-pbf-file",
      "--ls-endpoint",
      &first.url("/index.json"),
      "ls",
      "geofabrik",
    ],
  );
  assert_eq!(out.status, 0, "stderr: {}", out.stderr);
  assert!(!out.stdout.contains("gamma"));

  let second = stub::start(INDEX_WITH_GAMMA, vec![]);
  let endpoint = second.url("/index.json");
  let stale = w.geolite_in(
    &dir,
    &[
      "osm-pbf-file",
      "--ls-endpoint",
      &endpoint,
      "ls",
      "geofabrik",
    ],
  );
  assert_eq!(stale.status, 0, "stderr: {}", stale.stderr);
  assert!(
    !stale.stdout.contains("gamma"),
    "without --recreate-cache the cache wins"
  );

  let fresh = w.geolite_in(
    &dir,
    &[
      "osm-pbf-file",
      "--ls-endpoint",
      &endpoint,
      "ls",
      "geofabrik",
      "--recreate-cache",
    ],
  );
  assert_eq!(fresh.status, 0, "stderr: {}", fresh.stderr);
  assert!(fresh.stdout.contains("gamma"), "stdout: {}", fresh.stdout);
}

// 06. download
#[test]
#[ignore]
fn _06_download_by_geofabrik_id_records_a_geofabrik_origin_with_its_path() {
  let w = world();
  let dir = w.scratch("download_by_id");
  let bytes = fixture_bytes(w);
  let s = stub::start(INDEX, vec![("alpha.osm.pbf", bytes.clone())]);
  download(w, &dir, &s.url("/index.json"), "alpha");

  assert_eq!(
    std::fs::read(dir.join("alpha.osm.pbf")).expect("the download must exist"),
    bytes
  );
  let conn = database(&dir);
  let rows = ledger(&conn);
  let alpha = rows
    .iter()
    .find(|r| r.origin_id.as_deref() == Some("alpha"))
    .expect("alpha row");
  assert_eq!(alpha.origin, 1);
  assert_eq!(alpha.origin_name.as_deref(), Some("Alpha"));
  assert_eq!(alpha.url.as_deref(), Some(s.url("/alpha.osm.pbf").as_str()));
  assert_eq!(
    alpha.path.as_deref(),
    Some(joined(&dir, "alpha.osm.pbf").as_str())
  );
  let (size, md5): (i64, String) = conn
    .query_row(
      "SELECT size_bytes, md5 FROM osm_pbf_files WHERE origin_id = 'alpha'",
      [],
      |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .expect("failed to read the download columns");
  assert_eq!(size as usize, bytes.len());
  assert_eq!(md5, format!("{:x}", md5::compute(&bytes)));
  assert!(index_exists(&conn, "osm_pbf_files_search_by_url"));
}

// 07. download
#[test]
#[ignore]
fn _07_download_by_url_records_a_url_origin_that_the_next_ls_promotes_to_geofabrik() {
  let w = world();
  let dir = w.scratch("download_by_url");
  let s = stub::start(INDEX, vec![("alpha.osm.pbf", fixture_bytes(w))]);
  let url = s.url("/alpha.osm.pbf");
  download(w, &dir, &s.url("/index.json"), &url);

  let before = ledger(&database(&dir));
  assert_eq!(before.len(), 1);
  assert_eq!(before[0].origin, 2);
  assert_eq!(before[0].origin_id, None);
  assert_eq!(before[0].origin_name.as_deref(), Some("alpha.osm.pbf"));
  assert_eq!(before[0].url.as_deref(), Some(url.as_str()));

  let out = w.geolite_in(
    &dir,
    &[
      "osm-pbf-file",
      "--ls-endpoint",
      &s.url("/index.json"),
      "ls",
      "geofabrik",
    ],
  );
  assert_eq!(out.status, 0, "stderr: {}", out.stderr);
  let after = ledger(&database(&dir));
  let promoted = after
    .iter()
    .find(|r| r.id == before[0].id)
    .expect("the url row survives");
  assert_eq!(promoted.origin, 1);
  assert_eq!(promoted.origin_id.as_deref(), Some("alpha"));
  assert_eq!(promoted.origin_name.as_deref(), Some("Alpha"));
  assert_eq!(promoted.path, before[0].path);
  assert_eq!(after.iter().filter(|r| r.path == before[0].path).count(), 1);
}

// 08. extract then download
#[test]
#[ignore]
fn _08_a_file_extracted_before_being_downloaded_takes_the_download_origin() {
  let w = world();
  let dir = w.scratch("extract_then_download");
  let bytes = fixture_bytes(w);
  std::fs::write(dir.join("alpha.osm.pbf"), &bytes).expect("failed to copy the fixture");

  let out = w.geolite_in(&dir, &["extract", "osm-pbf-blob-chunks", "alpha.osm.pbf"]);
  assert_eq!(out.status, 0, "stderr: {}", out.stderr);
  let before = ledger(&database(&dir));
  assert_eq!(before.len(), 1);
  assert_eq!(before[0].origin, 0);
  assert_eq!(
    before[0].path.as_deref(),
    Some(joined(&dir, "alpha.osm.pbf").as_str())
  );

  let s = stub::start(INDEX, vec![("alpha.osm.pbf", bytes)]);
  let url = s.url("/alpha.osm.pbf");
  download(w, &dir, &s.url("/index.json"), &url);
  let after = ledger(&database(&dir));
  assert_eq!(after.len(), 1);
  assert_eq!(after[0].id, before[0].id);
  assert_eq!(after[0].origin, 2);
  assert_eq!(after[0].url.as_deref(), Some(url.as_str()));
  assert_eq!(after[0].path, before[0].path);

  let chunks = open_sqlite_at(&dir.join("database.osm_data.sqlite3"));
  assert!(
    count(
      &chunks,
      &format!(
        "SELECT COUNT(*) FROM osm_pbf_blob_chunks WHERE file_id = {}",
        after[0].id
      )
    ) > 0,
    "the blob chunks stay linked to the same ledger row"
  );
  assert!(index_exists(
    &chunks,
    "osm_pbf_blob_chunks_search_by_file_and_type"
  ));
}

// 09. resolution by origin id
#[test]
#[ignore]
fn _09_extract_resolves_a_downloaded_file_by_its_geofabrik_id() {
  let w = world();
  let dir = w.scratch("resolve_by_id");
  let s = stub::start(INDEX, vec![("alpha.osm.pbf", fixture_bytes(w))]);
  download(w, &dir, &s.url("/index.json"), "alpha");

  for stage in ["osm-pbf-blob-chunks", "osm-pbf-header"] {
    let out = w.geolite_in(&dir, &["extract", stage, "alpha"]);
    assert_eq!(
      out.status, 0,
      "extract {stage} alpha failed:\n{}",
      out.stderr
    );
  }
  let conn = database(&dir);
  let rows = ledger(&conn);
  let alpha: Vec<&row> = rows
    .iter()
    .filter(|r| r.origin_id.as_deref() == Some("alpha"))
    .collect();
  assert_eq!(alpha.len(), 1);
  assert_eq!(alpha[0].origin, 1);
  let program: Option<String> = conn
    .query_row(
      "SELECT osm_header_writingprogram FROM osm_pbf_files WHERE origin_id = 'alpha'",
      [],
      |r| r.get(0),
    )
    .expect("failed to read the header column");
  assert!(
    program.is_some(),
    "the header stage must write on the resolved row"
  );
}

// 10. build from a url
#[test]
#[ignore]
fn _10_build_from_a_url_downloads_and_runs_every_stage() {
  let w = world();
  let dir = w.scratch("build_from_url");
  let s = stub::start(INDEX, vec![("alpha.osm.pbf", fixture_bytes(w))]);
  let url = s.url("/alpha.osm.pbf");

  let out = w.geolite_in(
    &dir,
    &["--threads", "1", "--preset", "brazil", "build", &url],
  );
  assert_eq!(
    out.status, 0,
    "build {url} failed:\n{}\n{}",
    out.stdout, out.stderr
  );
  for banner in ["── download", "saved", "── extract blob-chunks"] {
    assert!(
      out.stdout.contains(banner),
      "stdout lacks {banner}:\n{}",
      out.stdout
    );
  }

  let conn = database(&dir);
  let rows = ledger(&conn);
  assert_eq!(rows.len(), 1);
  assert_eq!(rows[0].origin, 2);
  assert_eq!(rows[0].url.as_deref(), Some(url.as_str()));
  assert_eq!(
    rows[0].path, None,
    "optimize deleted the download, so the ledger must forget its path"
  );
  let download: (Option<i64>, Option<String>, Option<i64>) = conn
    .query_row(
      "SELECT size_bytes, md5, downloaded_at FROM osm_pbf_files",
      [],
      |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .expect("failed to read the download columns");
  assert_eq!(download, (None, None, None));
  for column in [
    "node_count",
    "way_count",
    "relation_count",
    "admin_levels_count",
    "house_numbers_count",
  ] {
    assert!(
      count(&conn, &format!("SELECT {column} FROM osm_pbf_files")) > 0,
      "{column} must be filled by the pipeline"
    );
  }
  assert!(dir.join("database.tantivy").join("meta.json").exists());
  assert!(!dir.join("database.osm_data.sqlite3").exists());
}

// 11. idempotent stage
#[test]
#[ignore]
fn _11_re_running_a_stage_without_recreate_keeps_one_ledger_row_and_one_chunk_set() {
  let w = world();
  let dir = w.scratch("rerun_blob_chunks");
  std::fs::copy(&w.pbf, dir.join("santos.osm.pbf")).expect("failed to copy the fixture");

  let mut seen = Vec::new();
  for _ in 0..2 {
    let out = w.geolite_in(&dir, &["extract", "osm-pbf-blob-chunks", "santos.osm.pbf"]);
    assert_eq!(out.status, 0, "stderr: {}", out.stderr);
    let rows = ledger(&database(&dir));
    assert_eq!(rows.len(), 1);
    let chunks = open_sqlite_at(&dir.join("database.osm_data.sqlite3"));
    seen.push((
      rows[0].id,
      count(
        &chunks,
        &format!(
          "SELECT COUNT(*) FROM osm_pbf_blob_chunks WHERE file_id = {}",
          rows[0].id
        ),
      ),
    ));
  }
  assert_eq!(
    seen[0], seen[1],
    "a re-run must keep the same row and chunk set"
  );
  assert!(seen[0].1 > 0);
}

// 12. cli contract
#[test]
#[ignore]
fn _12_osm_pbf_file_help_documents_the_endpoint_override_without_a_default() {
  let w = world();
  let out = w.geolite(&["osm-pbf-file", "--help"]);
  assert_eq!(out.status, 0, "stderr: {}", out.stderr);
  assert!(
    out.stdout.contains("--ls-endpoint"),
    "stdout: {}",
    out.stdout
  );
  assert!(
    out.stdout.contains("overrides the geofabrik index url"),
    "stdout: {}",
    out.stdout
  );
  assert!(
    !out.stdout.contains("download.geofabrik.de"),
    "the default endpoint belongs to the domain, not to the cli:\n{}",
    out.stdout
  );
}

// 13. coverage
#[test]
#[ignore]
fn _13_ls_caches_the_coverage_of_a_region_and_download_keeps_it() {
  let w = world();
  let dir = w.scratch("coverage_ls_download");
  let s = stub::start(
    INDEX_WITH_COVERAGE,
    vec![("alpha.osm.pbf", fixture_bytes(w))],
  );
  ls(w, &dir, &s.url("/index.json"));
  let alpha = coverage(&database(&dir), "alpha").expect("alpha has a geometry in the index");
  assert!(alpha.starts_with("MULTIPOLYGON"), "got {alpha}");
  assert!(
    alpha.contains("1 42") && alpha.contains("2 43"),
    "the polygon must keep its coordinates: {alpha}"
  );
  assert_eq!(
    coverage(&database(&dir), "beta"),
    None,
    "beta has no geometry in the index"
  );

  download(w, &dir, &s.url("/index.json"), "alpha");
  let conn = database(&dir);
  assert_eq!(
    coverage(&conn, "alpha").as_deref(),
    Some(alpha.as_str()),
    "the download must keep the coverage"
  );
  let downloaded = ledger(&conn)
    .into_iter()
    .find(|r| r.origin_id.as_deref() == Some("alpha"))
    .expect("alpha row");
  assert!(
    downloaded.path.is_some(),
    "the same row now carries the path"
  );
}

// 14. coverage on promotion
#[test]
#[ignore]
fn _14_a_url_download_receives_the_coverage_when_the_next_ls_promotes_it() {
  let w = world();
  let dir = w.scratch("coverage_promotion");
  let s = stub::start(
    INDEX_WITH_COVERAGE,
    vec![("alpha.osm.pbf", fixture_bytes(w))],
  );
  download(w, &dir, &s.url("/index.json"), &s.url("/alpha.osm.pbf"));
  {
    let conn = database(&dir);
    let rows = ledger(&conn);
    assert_eq!((rows.len(), rows[0].origin), (1, 2));
    assert_eq!(
      count(
        &conn,
        "SELECT COUNT(*) FROM osm_pbf_files WHERE origin_wkt IS NOT NULL"
      ),
      0,
      "a url download knows no coverage"
    );
  }
  ls(w, &dir, &s.url("/index.json"));
  let conn = database(&dir);
  // ls also caches beta, so the promoted alpha row is not the only one; the download row is.
  let rows = ledger(&conn);
  let downloaded: Vec<&row> = rows.iter().filter(|r| r.path.is_some()).collect();
  assert_eq!(downloaded.len(), 1, "only the downloaded file has a path");
  assert_eq!(
    downloaded[0].origin, 1,
    "the url row was promoted to geofabrik"
  );
  assert_eq!(downloaded[0].origin_id.as_deref(), Some("alpha"));
  assert!(
    coverage(&conn, "alpha").is_some_and(|wkt| wkt.starts_with("MULTIPOLYGON")),
    "the promotion must bring the coverage along"
  );
}

// 15. additive migration
#[test]
#[ignore]
fn _15_a_database_without_origin_wkt_gains_it_on_the_next_write_command() {
  let w = world();
  let dir = w.scratch("coverage_migration");
  std::fs::copy(&w.sqlite_path, dir.join("database.sqlite3")).expect("failed to copy the database");
  let has_column = |conn: &rusqlite::Connection| {
    count(
      conn,
      "SELECT COUNT(*) FROM pragma_table_info('osm_pbf_files') WHERE name = 'origin_wkt'",
    ) == 1
  };
  let nodes_before = {
    let conn = rusqlite::Connection::open(dir.join("database.sqlite3")).expect("failed to open");
    conn
      .execute_batch("ALTER TABLE osm_pbf_files DROP COLUMN origin_wkt")
      .expect("failed to drop the column");
    assert!(
      !has_column(&conn),
      "the copy now looks like a 0.0.6 database"
    );
    count(&conn, "SELECT node_count FROM osm_pbf_files")
  };
  let s = stub::start(INDEX_WITH_COVERAGE, vec![]);
  ls(w, &dir, &s.url("/index.json"));
  let conn = database(&dir);
  assert!(
    has_column(&conn),
    "the write command must add the column back"
  );
  assert_eq!(
    count(&conn, "PRAGMA user_version"),
    2,
    "no version bump for an added column"
  );
  let santos = ledger(&conn)
    .into_iter()
    .find(|r| r.origin == 0)
    .expect("the local build row survives");
  assert_eq!(santos.origin_name.as_deref(), Some("santos.osm.pbf"));
  assert_eq!(
    count(
      &conn,
      "SELECT node_count FROM osm_pbf_files WHERE origin = 0"
    ),
    nodes_before,
    "the migration keeps every value of the old rows"
  );
  assert!(coverage(&conn, "alpha").is_some());
}
