use crate::cli::index::command_handler_index;
use crate::cli::optimize::command_handler_optimize;
use crate::database::admin_levels::{admin_levels as admin_levels_row, batch_upsert};
use crate::database::{open_write, osm_data_path};
use crate::index::admin_levels_hierarchy_tantivy as tantivy;
use crate::presets::{BRAZIL, DEFAULT};
use crate::query;
use geo::{Coord, Geometry, LineString};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

// these tests drive the real index + query + optimize code on synthetic admin_levels (the same
// pattern as text_search.test.rs / merge.test.rs). the abbreviation expansion is name folding and
// the osm_data sibling is file plumbing — neither needs real geography — so no pbf fixture is used.

fn unique_tag() -> String {
  static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
  let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
  format!("{}_{seq}", std::process::id())
}

// an isolated on-disk workspace: a base dir holding the (optional) main sqlite path, its osm_data
// sibling, and the tantivy index dir. Drop removes the whole base dir in one shot.
struct workspace {
  base: PathBuf,
  sqlite_path: String,
  index_path: String,
}

impl workspace {
  fn new(tag: &str) -> Self {
    let base = std::env::temp_dir().join(format!("geolite_build_test_{tag}_{}", unique_tag()));
    std::fs::create_dir_all(&base).expect("failed to create workspace base dir");
    workspace {
      sqlite_path: base.join("db.sqlite3").to_string_lossy().into_owned(),
      index_path: base.join("index.tantivy").to_string_lossy().into_owned(),
      base,
    }
  }
}

impl Drop for workspace {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.base);
  }
}

// a synthetic level-12 street with a tiny geometry at a distinct location (lon_offset keeps streets
// spatially separate so the hierarchy/rtree treat them as different rows).
fn make_street(name: &str, way_id: u64, lon_offset: f64) -> admin_levels_row {
  admin_levels_row {
    relation_id: None,
    way_id: Some(way_id),
    admin_level: 12,
    wkb: Geometry::LineString(LineString(vec![
      Coord { x: -46.3198 + lon_offset, y: -23.9724 },
      Coord { x: -46.3197 + lon_offset, y: -23.9724 },
    ]))
    .into(),
    name: name.to_string(),
    country_iso_code: None,
    post_code: None,
  }
}

// builds an in-memory db holding `streets` (level 12) and a tantivy index folded with
// `abbreviations`. brazil and default share the same boosts — only the abbreviation table differs.
fn synthetic_index(
  work: &workspace,
  streets: &[&str],
  abbreviations: &[(&str, &str)],
) -> (Connection, tantivy::tantivy_index) {
  let conn = open_write(":memory:");
  let rows: Vec<admin_levels_row> = streets
    .iter()
    .enumerate()
    .map(|(i, name)| make_street(name, (i + 1) as u64, i as f64 * 0.0005))
    .collect();
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let index = tantivy::build(
    &conn,
    Path::new(&work.index_path),
    DEFAULT.index_user_friendly_name.boosts,
    abbreviations,
  );
  (conn, index)
}

fn query_text(conn: &Connection, index: &tantivy::tantivy_index, q: &str) -> query::query_output {
  query::run(conn, Some(index), q, None, None, None, None, false)
}

// the most specific admin_level of a match is the matched street (highest level number).
fn matched_street_name(m: &query::query_match) -> Option<&str> {
  m.admin_levels.iter().max_by_key(|a| a.level).map(|a| a.name.as_str())
}

fn top_street(out: &query::query_output) -> Option<&str> {
  out.matches.first().and_then(matched_street_name)
}

// with --preset brazil the user-friendly-name index folds the abbreviation table into each document,
// so an abbreviated query resolves to the full street form. exercises three distinct abbreviations
// (r./av./pç.) through index -> search.
#[test]
fn _00_brazil_preset_expands_abbreviations_in_tantivy_index() {
  let work = workspace::new("brazil_abbrev");
  let (conn, index) = synthetic_index(
    &work,
    &["Rua Braz Cubas", "Avenida Ana Costa", "Praça dos Andradas"],
    BRAZIL.index_user_friendly_name.abbreviations,
  );

  for (abbreviated, full) in [
    ("R. Braz Cubas", "Rua Braz Cubas"),
    ("Av. Ana Costa", "Avenida Ana Costa"),
    ("Pç. dos Andradas", "Praça dos Andradas"),
  ] {
    assert_eq!(
      top_street(&query_text(&conn, &index, abbreviated)),
      Some(full),
      "abbreviated query '{abbreviated}' should resolve to '{full}'"
    );
  }
}

// the regression guard for the brazil rules. "São Francisco" exists as both a rua and an avenida, so
// the leading "r."/"av." is the only thing that can pick the right one. under --preset brazil the
// abbreviation table folds those tokens into the documents and both directions resolve correctly. the
// default preset (empty table) can't expand the type token, so both queries carry the same
// distinctive tokens and it cannot map them to different streets — emptying or breaking BRAZIL's
// abbreviations makes the brazil build behave the same and fails this test.
#[test]
fn _01_default_preset_does_not_expand_abbreviations() {
  let streets = ["Rua São Francisco", "Avenida São Francisco"];

  let bw = workspace::new("brazil_disambig");
  let (bconn, bindex) = synthetic_index(&bw, &streets, BRAZIL.index_user_friendly_name.abbreviations);
  assert_eq!(
    top_street(&query_text(&bconn, &bindex, "R. São Francisco")),
    Some("Rua São Francisco"),
    "brazil preset should disambiguate 'R. São Francisco' to the rua"
  );
  assert_eq!(
    top_street(&query_text(&bconn, &bindex, "Av. São Francisco")),
    Some("Avenida São Francisco"),
    "brazil preset should disambiguate 'Av. São Francisco' to the avenida"
  );

  let dw = workspace::new("default_disambig");
  let (dconn, dindex) = synthetic_index(&dw, &streets, DEFAULT.index_user_friendly_name.abbreviations);
  let default_rua = top_street(&query_text(&dconn, &dindex, "R. São Francisco")).map(str::to_string);
  let default_av = top_street(&query_text(&dconn, &dindex, "Av. São Francisco")).map(str::to_string);
  assert!(
    !(default_rua.as_deref() == Some("Rua São Francisco")
      && default_av.as_deref() == Some("Avenida São Francisco")),
    "default preset has no abbreviation table and must not disambiguate both directions \
     (rua={default_rua:?}, av={default_av:?})"
  );
}

// the osm_data.sqlite3 sibling holds raw extracted nodes/ways/relations. it is created the moment the
// build opens the database for writing (open_write attaches and creates the osm_data tables), and it
// must be deleted by the optimize step (delete-intermediary-data). this is file plumbing, so it runs
// on synthetic admin_levels. the index dir is the only on-disk artifact besides the db.
#[test]
fn _02_build_creates_then_deletes_osm_data_sibling() {
  let work = workspace::new("osm_data_lifecycle");

  // open_write creates the osm_data sibling; a couple of streets with geometry plus the derived
  // hierarchy (built by command_handler_index below) satisfy the optimize preconditions.
  {
    let conn = open_write(&work.sqlite_path);
    batch_upsert(&conn, &[make_street("way_1", 1, 0.0), make_street("way_2", 2, 0.0005)]);
  }

  let sibling = osm_data_path(&work.sqlite_path);
  assert!(
    Path::new(&sibling).exists(),
    "osm_data sibling must exist after the database is opened for writing"
  );

  command_handler_index(&work.sqlite_path, &work.index_path, None, &DEFAULT.index_user_friendly_name);
  command_handler_optimize(&work.base.to_string_lossy(), &work.sqlite_path, &work.index_path, None);

  assert!(
    !Path::new(&sibling).exists(),
    "osm_data sibling must be deleted after optimize"
  );
}

// serves a fixed json body on every request — the geofabrik index stub for build scenarios.
fn start_json_server(body: &'static str) -> String {
  use std::io::{Read, Write};
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

// a fixture pbf with two street nodes, one house-number node and the street way itself: the
// smallest input where every build stage (download probe, blob-chunks, header, osm-data,
// admin-levels, house-numbers, index, optimize) has real work to do.
fn write_street_fixture(pbf_path: &str) {
  use crate::extract::pbf_fixtures::{
    blob_compression, block_spec, data_chunk, header_chunk, node, way, write_pbf,
  };
  write_pbf(
    pbf_path,
    &[
      header_chunk(),
      data_chunk(
        &block_spec {
          dense: vec![
            node(1, -23.9724, -46.3198, &[]),
            node(2, -23.9724, -46.3197, &[]),
            node(3, -23.97235, -46.31975, &[("addr:housenumber", "10"), ("addr:street", "Rua Teste")]),
          ],
          ..Default::default()
        },
        blob_compression::zlib,
      ),
      data_chunk(
        &block_spec {
          ways: vec![way(100, &[1, 2], &[("highway", "residential"), ("name", "Rua Teste")])],
          ..Default::default()
        },
        blob_compression::zlib,
      ),
    ],
  );
}

// drives command_handler_build end to end on a local fixture: the source is not a geofabrik id
// (the stub index is empty, so download only warns), every extract stage runs on real data, and
// the pipeline ends with a populated admin_levels, a tantivy dir and no osm_data sibling.
#[test]
fn _03_full_pipeline_from_local_pbf_fixture_runs_every_stage() {
  let work = workspace::new("full_pipeline");
  let pbf_path = work.base.join("fixture.osm.pbf").to_string_lossy().into_owned();
  write_street_fixture(&pbf_path);
  let ls_endpoint = start_json_server(r#"{"features":[]}"#);

  super::command_handler_build(
    &work.base.to_string_lossy(),
    &2,
    &work.sqlite_path,
    &work.index_path,
    &pbf_path,
    &ls_endpoint,
    false,
    &DEFAULT,
  );

  let conn = crate::database::open_readonly(&work.sqlite_path);
  assert!(
    crate::database::admin_levels::count_with_geometry(&conn) >= 1,
    "the street way must land in admin_levels"
  );
  assert!(
    Path::new(&work.index_path).exists(),
    "the tantivy index dir must be built"
  );
  assert!(
    !Path::new(&osm_data_path(&work.sqlite_path)).exists(),
    "the osm_data sibling must be deleted by the optimize stage"
  );
}

#[test]
#[ignore] // executed only as a child of _04
fn _90_build_source_file_not_found() {
  let work = workspace::new("missing_source");
  super::command_handler_build(
    &work.base.to_string_lossy(),
    &1,
    &work.sqlite_path,
    &work.index_path,
    "./nope/missing.osm.pbf",
    "http://127.0.0.1:1/index.json",
    false,
    &DEFAULT,
  );
}

#[test]
fn _04_build_missing_source_file_exits_one() {
  let out = crate::cli::tests::respawn("cli::build::tests::_90_build_source_file_not_found", &[], &[]);
  assert_eq!(out.status.code(), Some(1), "stderr: {}", crate::cli::tests::stderr_of(&out));
  assert!(crate::cli::tests::stderr_of(&out).contains("source file not found"));
}

#[test]
#[ignore] // executed only as a child of _05
fn _90_build_unresolvable_source() {
  let work = workspace::new("unresolvable_source");
  let ls_endpoint = start_json_server(r#"{"features":[]}"#);
  super::command_handler_build(
    &work.base.to_string_lossy(),
    &1,
    &work.sqlite_path,
    &work.index_path,
    "unknown-id",
    &ls_endpoint,
    false,
    &DEFAULT,
  );
}

#[test]
fn _05_build_unresolvable_source_exits_one_after_download() {
  let out = crate::cli::tests::respawn("cli::build::tests::_90_build_unresolvable_source", &[], &[]);
  assert_eq!(out.status.code(), Some(1), "stderr: {}", crate::cli::tests::stderr_of(&out));
  assert!(
    crate::cli::tests::stderr_of(&out).contains("could not resolve source after download")
  );
}
