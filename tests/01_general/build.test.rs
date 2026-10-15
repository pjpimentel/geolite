use std::path::{Path, PathBuf};

use crate::common::harness::{open_scratch, plain, query_at, world};
use crate::extract::{REGENERATE, copy_fixture, extracted, stage};
use crate::general::world;
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

// the levels asked for brought from one source into the bases of the data path, with the house
// numbers when the streets are among them
fn brought(w: &world, dir: &Path, source: &str, levels: &str) {
  stage(
    w,
    dir,
    &[
      "exec",
      "extract-osm-admin-levels",
      source,
      "--admin-level",
      levels,
    ],
  );
  if levels.contains("12") {
    stage(w, dir, &["exec", "extract-osm-house-numbers", source]);
  }
}

// the fixture a second time under another name, through its three pbf stages: a second source in
// the same data path
fn second_source(w: &world, dir: &Path) -> String {
  let pbf = copy_fixture(w, dir, "santos-again.osm.pbf");
  stage(w, dir, &["exec", "extract-osm-pbf-blob-chunks", &pbf]);
  stage(w, dir, &["exec", "extract-osm-pbf-header", &pbf]);
  stage(
    w,
    dir,
    &["--threads", "2", "exec", "extract-osm-pbf-data", &pbf],
  );
  "santos-again.osm.pbf".to_string()
}

fn combined(w: &world, name: &str) -> PathBuf {
  let dir = extracted(w, name, "2", &[]).dir;
  brought(w, &dir, "santos.osm.pbf", "2,4,8,10,12");
  indexed_and_merged(w, &dir);
  dir
}

// 00.00. two sources into one data path, each bringing part of the levels, answer like a single
// build of the whole: a base grows source by source
#[test]
#[ignore]
fn _00_00_two_sources_into_one_data_path_match_a_single_build() {
  let w = world();
  let dir = extracted(w, "build_two_sources", "2", &[]).dir;
  let again = second_source(w, &dir);
  brought(w, &dir, "santos.osm.pbf", "2,4,8");
  brought(w, &dir, &again, "10,12");
  indexed_and_merged(w, &dir);

  let single = combined(w, "build_single_source");

  assert_eq!(signature(&dir), signature(&single), "{REGENERATE}");
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
  brought(w, &dir, "santos.osm.pbf", "2,4,8,10,12");
  let out = stage(
    w,
    &dir,
    &[
      "exec",
      "extract-osm-admin-levels",
      "santos.osm.pbf",
      "--admin-level",
      "2,4,8,10,12",
    ],
  );
  assert!(
    out.stdout.contains("done (0 remaining)"),
    "the second pass finds every relation already brought:\n{}",
    out.stdout
  );
  brought(w, &dir, "santos.osm.pbf", "12");
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
    plain(&built.stdout).contains("sealed br.sqlite3, br.tantivy — read-only"),
    "{}",
    built.stdout
  );
  let before = signature(&dir);

  let again = second_source(w, &dir);
  let out = stage(
    w,
    &dir,
    &[
      "exec",
      "extract-osm-admin-levels",
      &again,
      "--admin-level",
      "2,12",
    ],
  );
  let stdout = plain(&out.stdout);
  assert!(
    stdout.contains("sealed br.sqlite3 — takes no row"),
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
    plain(&refused.stderr).contains("br.sqlite3 is sealed"),
    "{}",
    refused.stderr
  );
}
