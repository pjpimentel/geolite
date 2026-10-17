use std::path::{Path, PathBuf};

use crate::common::harness::{open_scratch, plain, query_at, world};
use crate::extract::{REGENERATE, copy_fixture, count, extracted, stage};
use crate::general::world;
use crate::house_number::HOUSE_NUMBERS;
use crate::street_merge::STREETS;
use crate::street_merge::indexed_and_merged;

const TEXT_QUERY: &str = "rua januario dos santos, santos 197";
const COORDINATES_QUERY: &str = "-23.98202,-46.31005";

// the rows a data path holds, as three strings: equal strings mean the same data landed
fn signature(dir: &Path) -> (String, String, String) {
  let conn = open_scratch(dir);
  let read = |sql: &str| -> String {
    conn
      .query_row(sql, [], |r| r.get::<_, Option<String>>(0))
      .expect("failed to read the signature")
      .unwrap_or_default()
  };
  (
    read(
      "SELECT GROUP_CONCAT(admin_level || ':' || rows) FROM \
       (SELECT admin_level, COUNT(*) AS rows FROM admin_levels GROUP BY admin_level ORDER BY admin_level)",
    ),
    read(
      "SELECT GROUP_CONCAT(admin_level_id || '>' || COALESCE(parent_id, '')) FROM \
       (SELECT admin_level_id, parent_id FROM admin_levels_hierarchy ORDER BY admin_level_id, parent_id)",
    ),
    read("SELECT GROUP_CONCAT(node_id) FROM (SELECT node_id FROM house_numbers ORDER BY node_id)"),
  )
}

// the levels asked for brought from the osm_data into the bases of the data path, with the house
// numbers when the streets are among them
fn brought(w: &world, dir: &Path, levels: &str) {
  stage(
    w,
    dir,
    &["exec", "extract-osm-admin-levels", "--admin-level", levels],
  );
  if levels.contains("12") {
    stage(w, dir, &["exec", "extract-osm-house-numbers"]);
  }
}

// the fixture a second time under another name, through its three pbf stages: a second source in
// the same osm_data
fn second_source(w: &world, dir: &Path) {
  let pbf = copy_fixture(w, dir, "santos-again.osm.pbf");
  stage(w, dir, &["exec", "extract-osm-pbf-blob-chunks", &pbf]);
  stage(w, dir, &["exec", "extract-osm-pbf-header", &pbf]);
  stage(
    w,
    dir,
    &["--threads", "2", "exec", "extract-osm-pbf-data", &pbf],
  );
}

fn combined(w: &world, name: &str) -> PathBuf {
  let dir = extracted(w, name, "2", &[]).dir;
  brought(w, &dir, "2,4,8,10,12");
  indexed_and_merged(w, &dir);
  dir
}

// the count a ledger row carries, by the name of its file
fn ledger_count_of(dir: &Path, column: &str, origin_name: &str) -> i64 {
  count(
    &open_scratch(dir),
    &format!("SELECT {column} FROM osm_pbf_files WHERE origin_name = '{origin_name}'"),
  )
}

// 00.00. two sources in one osm_data, the levels brought in two runs, answer like a single build
// of the whole; the elements the second source repeats are stored once and credited to the first
#[test]
#[ignore]
fn _00_00_two_sources_into_one_data_path_match_a_single_build() {
  let w = world();
  let dir = extracted(w, "build_two_sources", "2", &[]).dir;
  second_source(w, &dir);
  brought(w, &dir, "2,4,8");
  brought(w, &dir, "10,12");
  indexed_and_merged(w, &dir);

  let single = combined(w, "build_single_source");

  assert_eq!(signature(&dir), signature(&single), "{REGENERATE}");
  assert!(ledger_count_of(&dir, "admin_levels_count", "santos.osm.pbf") > 0);
  assert_eq!(
    ledger_count_of(&dir, "admin_levels_count", "santos-again.osm.pbf"),
    0,
    "the second source repeats the first: stored once, credited to the first"
  );
  for query in [TEXT_QUERY, COORDINATES_QUERY] {
    assert_eq!(
      query_at(w, &dir, query),
      query_at(w, &single, query),
      "the data path of two sources must answer {query:?} like the single build"
    );
  }
}

// 00.01. the same source twice writes the same rows: admin levels upsert by id, house numbers
// ignore a node id they already have, and the resume brings nothing the base holds
#[test]
#[ignore]
fn _00_01_bringing_the_same_source_twice_changes_nothing() {
  let w = world();
  let once = combined(w, "build_once");

  let dir = extracted(w, "build_twice", "2", &[]).dir;
  brought(w, &dir, "2,4,8,10,12");
  let out = stage(
    w,
    &dir,
    &[
      "exec",
      "extract-osm-admin-levels",
      "--admin-level",
      "2,4,8,10,12",
    ],
  );
  assert!(
    out.stdout.contains("done (0 remaining)"),
    "the second pass finds every relation already brought:\n{}",
    out.stdout
  );
  brought(w, &dir, "12");
  indexed_and_merged(w, &dir);

  assert_eq!(signature(&once), signature(&dir));
}

// 00.02. a base built once is final: `build` seals it, a later source for the same country is
// dropped aloud and leaves it as it was, and a write stage named on it is refused
#[test]
#[ignore]
fn _00_02_a_sealed_base_takes_no_row_from_a_later_source() {
  let w = world();
  let dir = w.scratch("build_sealed");
  let pbf = copy_fixture(w, &dir, "santos.osm.pbf");
  let built = w.geolite_in(&dir, &["--threads", "2", "build", &pbf]);
  assert_eq!(built.status, 0, "{}", built.stderr);
  assert!(
    plain(&built.stdout).contains("sealed br/data.sqlite3, br/index.tantivy — read-only"),
    "{}",
    built.stdout
  );
  let before = signature(&dir);
  let refused = w.geolite_in(
    &dir,
    &["exec", "extract-osm-admin-levels", "--admin-level", "2"],
  );
  assert_eq!(refused.status, 1, "{}", refused.stdout);
  assert!(
    plain(&refused.stderr).contains("extract-osm-admin-levels requires extract-osm-pbf-data"),
    "the build deleted the osm_data, so there is nothing to extract from: {}",
    refused.stderr
  );

  second_source(w, &dir);
  let out = stage(
    w,
    &dir,
    &["exec", "extract-osm-admin-levels", "--admin-level", "2,12"],
  );
  let stdout = plain(&out.stdout);
  assert!(
    stdout.contains("sealed br/data.sqlite3 — takes no row"),
    "{stdout}"
  );
  assert!(
    stdout.contains("dropped ") && stdout.contains("street — the base of their country is sealed"),
    "{stdout}"
  );
  assert_eq!(signature(&dir), before, "the sealed base is as it was");

  let refused = w.geolite_in(&dir, &["exec", "index-addresses", "br"]);
  assert_eq!(refused.status, 1, "{}", refused.stdout);
  assert!(
    plain(&refused.stderr).contains("br/data.sqlite3 is sealed"),
    "{}",
    refused.stderr
  );
}

// 00.03. a build of two sources runs the pbf stages once per source and everything after them
// once: one extraction over the shared osm_data, one index, one optimize, one seal
#[test]
#[ignore]
fn _00_03_a_build_of_two_sources_runs_the_pbf_stages_per_source_and_the_rest_once() {
  let w = world();
  let dir = w.scratch("build_two_sources_at_once");
  let santos = copy_fixture(w, &dir, "santos.osm.pbf");
  let again = copy_fixture(w, &dir, "santos-again.osm.pbf");
  let out = w.geolite_in(&dir, &["--threads", "2", "build", &santos, &again]);
  assert_eq!(out.status, 0, "{}", out.stderr);
  let stdout = plain(&out.stdout);
  for line in [
    "extracted 94 chunks from santos.osm.pbf",
    "extracted 94 chunks from santos-again.osm.pbf",
    "santos.osm.pbf  nodes: 683.3K",
    "santos-again.osm.pbf  nodes: 683.3K",
    "deleted santos.osm.pbf",
    "deleted santos-again.osm.pbf",
    "deleted osm_data.sqlite3",
    "deleted 3 tables in",
    "sealed br/data.sqlite3, br/index.tantivy — read-only",
  ] {
    assert!(stdout.contains(line), "{line:?} missing:\n{stdout}");
  }
  assert_eq!(stdout.matches("skipping download").count(), 2);
  assert_eq!(stdout.matches("── extract-osm-admin-levels").count(), 1);
  assert_eq!(stdout.matches("── extract-osm-house-numbers").count(), 1);

  assert_eq!(
    count(&open_scratch(&dir), "SELECT COUNT(*) FROM osm_pbf_files"),
    2
  );
  assert_eq!(
    count(
      &open_scratch(&dir),
      "SELECT COUNT(*) FROM osm_pbf_files WHERE path IS NOT NULL"
    ),
    0,
    "the build deleted both pbfs and the ledger forgot their paths"
  );
  assert_eq!(
    ledger_count_of(&dir, "admin_levels_count", "santos.osm.pbf"),
    1 + 27 + 12 + 63 + STREETS
  );
  assert_eq!(
    ledger_count_of(&dir, "house_numbers_count", "santos.osm.pbf"),
    HOUSE_NUMBERS
  );
  assert_eq!(
    ledger_count_of(
      &dir,
      "admin_levels_count + house_numbers_count",
      "santos-again.osm.pbf"
    ),
    0,
    "the second source repeats the first: stored once, credited to the first"
  );
  assert_eq!(
    query_at(w, &dir, TEXT_QUERY),
    query_at(w, &w.data_path, TEXT_QUERY),
    "two copies of the fixture build what one does"
  );
  assert!(!dir.join("osm_data.sqlite3").exists());
}
