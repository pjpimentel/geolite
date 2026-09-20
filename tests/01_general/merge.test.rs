use std::path::{Path, PathBuf};

use crate::common::harness::{merged_way_ids_of, open_sqlite_at, output, query_at, world};
use crate::extract::{REGENERATE, extracted, stage};
use crate::general::world;
use crate::street_merge::{LOWER_WAY, UPPER_WAY, drop_merged_way_ids, indexed_and_merged, way};

const TEXT_QUERY: &str = "rua januario dos santos, santos 197";
const COORDINATES_QUERY: &str = "-23.98202,-46.31005";

fn sqlite_of(dir: &Path) -> PathBuf {
  dir.join("database.sqlite3")
}

// the rows a build holds, as three strings: equal strings mean the merge landed the same data
fn signature(dir: &Path) -> (String, String, String) {
  let conn = crate::common::harness::open_sqlite_at(&sqlite_of(dir));
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

// a build of the levels asked for, with the house numbers when the streets are among them
fn half(w: &world, name: &str, levels: &str) -> PathBuf {
  let s = extracted(w, name, "2", &[]);
  stage(
    w,
    &s.dir,
    &[
      "--preset",
      "brazil",
      "extract",
      "osm-admin-levels",
      "--admin-level",
      levels,
    ],
  );
  if levels.contains("12") {
    stage(
      w,
      &s.dir,
      &["--preset", "brazil", "extract", "osm-house-numbers"],
    );
  }
  s.dir
}

fn merge(w: &world, base_dir: &Path, sources: &[&Path]) -> output {
  let base = sqlite_of(base_dir).to_string_lossy().into_owned();
  let mut args = vec![
    "--preset".to_string(),
    "brazil".to_string(),
    "merge".to_string(),
    base,
  ];
  args.extend(
    sources
      .iter()
      .map(|s| sqlite_of(s).to_string_lossy().into_owned()),
  );
  w.geolite_in(
    base_dir,
    &args.iter().map(String::as_str).collect::<Vec<_>>(),
  )
}

// the ways the street of two ways is traced to, in a base merged from one source
fn trace_after_merge(w: &world, base: &str, source: &Path) -> Option<Vec<u64>> {
  let base_dir = w.scratch(base);
  let merged = merge(w, &base_dir, &[source]);
  assert_eq!(merged.status, 0, "merge failed:\n{}", merged.stderr);
  merged_way_ids_of(&open_sqlite_at(&sqlite_of(&base_dir)), way(LOWER_WAY))
}

fn stamp_foreign_version(path: &Path) {
  let conn = rusqlite::Connection::open(path).expect("failed to create the sqlite file");
  conn
    .pragma_update(None, "user_version", 1)
    .expect("failed to stamp user_version");
}

// 00.00. a base merged from two halves answers like a single build of the whole
#[test]
#[ignore]
fn _00_00_a_merge_of_two_halves_matches_a_single_build() {
  let w = world();
  let areas = half(w, "merge_half_areas", "2,4,8");
  let streets = half(w, "merge_half_streets", "10,12");
  let base_dir = w.scratch("merge_base");
  let merged = merge(w, &base_dir, &[&areas, &streets]);
  assert_eq!(merged.status, 0, "merge failed:\n{}", merged.stderr);

  let combined = half(w, "merge_combined", "2,4,8,10,12");
  indexed_and_merged(w, &combined);

  assert_eq!(signature(&base_dir), signature(&combined), "{REGENERATE}");
  for query in [TEXT_QUERY, COORDINATES_QUERY] {
    assert_eq!(
      query_at(w, &base_dir, "brazil", query),
      query_at(w, &combined, "brazil", query),
      "the merged base must answer {query:?} like the single build"
    );
  }
}

// 00.01. the same source twice writes the same rows: admin levels upsert by id, house numbers
// ignore a node id they already have
#[test]
#[ignore]
fn _00_01_merging_the_same_source_twice_changes_nothing() {
  let w = world();
  let areas = half(w, "merge_twice_areas", "2,4,8");
  let streets = half(w, "merge_twice_streets", "10,12");

  let once = w.scratch("merge_once_base");
  assert_eq!(merge(w, &once, &[&areas, &streets]).status, 0);

  let twice = w.scratch("merge_twice_base");
  assert_eq!(merge(w, &twice, &[&areas, &streets, &streets]).status, 0);

  assert_eq!(signature(&once), signature(&twice));
}

// 00.02. a street folded in the source arrives with its trace: the base finds nothing left to fold,
// so the ways can only have come with the row
#[test]
#[ignore]
fn _00_02_a_folded_street_keeps_its_merged_way_ids_through_a_merge() {
  let w = world();
  let source = half(w, "merge_traced_source", "2,4,8,10,12");
  indexed_and_merged(w, &source);

  assert_eq!(
    trace_after_merge(w, "merge_traced_base", &source),
    Some(vec![LOWER_WAY, UPPER_WAY])
  );
}

// 00.03. a source built before the column still merges, and the base folds and traces its streets
#[test]
#[ignore]
fn _00_03_a_source_without_merged_way_ids_still_merges() {
  let w = world();
  let source = half(w, "merge_untraced_source", "2,4,8,10,12");
  drop_merged_way_ids(&source);

  assert_eq!(
    trace_after_merge(w, "merge_untraced_base", &source),
    Some(vec![LOWER_WAY, UPPER_WAY])
  );
}

// 01.00. the command needs at least one source: the parser takes an empty list, so the command
// itself is what refuses it, before any file is created
#[test]
#[ignore]
fn _01_00_merge_without_a_source_exits_one() {
  let w = world();
  let dir = w.scratch("merge_no_source");
  let base = sqlite_of(&dir);
  let out = w.geolite_in(&dir, &["merge", &base.to_string_lossy()]);
  assert_eq!(out.status, 1);
  assert!(
    out.stderr.contains("no databases to merge"),
    "stderr: {}",
    out.stderr
  );
  assert!(!base.exists(), "a refused merge creates no base");
}

// 01.01. a source that is not there
#[test]
#[ignore]
fn _01_01_merge_with_a_missing_database_exits_one() {
  let w = world();
  let dir = w.scratch("merge_missing_source");
  let out = merge(w, &dir, &[&dir.join("absent")]);
  assert_eq!(out.status, 1);
  assert!(
    out.stderr.contains("database not found"),
    "stderr: {}",
    out.stderr
  );
}

// 01.02. a base built by another schema version
#[test]
#[ignore]
fn _01_02_an_incompatible_base_exits_one() {
  let w = world();
  let areas = half(w, "merge_foreign_base_source", "2,4,8");
  let dir = w.scratch("merge_foreign_base");
  stamp_foreign_version(&sqlite_of(&dir));

  let out = merge(w, &dir, &[&areas]);
  assert_eq!(out.status, 1);
  assert!(
    out.stderr.contains("incompatible schema version on base"),
    "stderr: {}",
    out.stderr
  );
}

// 01.03. a source built by another schema version
#[test]
#[ignore]
fn _01_03_an_incompatible_source_exits_one() {
  let w = world();
  let source = w.scratch("merge_foreign_source");
  stamp_foreign_version(&sqlite_of(&source));
  let dir = w.scratch("merge_foreign_source_base");

  let out = merge(w, &dir, &[&source]);
  assert_eq!(out.status, 1);
  assert!(
    out.stderr.contains("incompatible schema version on source"),
    "stderr: {}",
    out.stderr
  );
}

// 01.04. a merge that lands no area at all
#[test]
#[ignore]
fn _01_04_a_merge_that_stays_empty_exits_one() {
  let w = world();
  let empty = extracted(w, "merge_empty_source", "2", &[]).dir;
  let dir = w.scratch("merge_empty_base");

  let out = merge(w, &dir, &[&empty]);
  assert_eq!(out.status, 1);
  assert!(
    out.stderr.contains("admin_levels is empty after merge"),
    "stderr: {}",
    out.stderr
  );
}
