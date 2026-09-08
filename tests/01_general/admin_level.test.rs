use std::path::Path;

use crate::common::harness::{output, world};
use crate::common::query::{first, levels_of, matches, name_at};
use crate::extract::{REGENERATE, count, extracted, scratch, stage};
use crate::general::world;
use serde_json::Value;

const SANTOS_RELATION: u64 = 298_442;
const SANTOS_ID: i64 = 596_885;
const COUNTRY_STATE_CITY: i64 = 40;
const EVERY_LEVEL: i64 = 12_981;

fn admin_levels_at(w: &world, dir: &Path, levels: &str, extra: &[&str]) -> output {
  let mut args = vec![
    "--preset",
    "brazil",
    "extract",
    "osm-admin-levels",
    "--admin-level",
    levels,
  ];
  args.extend_from_slice(extra);
  stage(w, dir, &args)
}

fn index_at(w: &world, dir: &Path, stages: &[&str]) -> output {
  let mut args = vec!["--preset", "brazil", "index"];
  args.extend_from_slice(stages);
  stage(w, dir, &args)
}

fn query_at(w: &world, dir: &Path, text: &str) -> Value {
  let out = w.geolite_in(
    dir,
    &[
      "--preset",
      "brazil",
      "query",
      text,
      "--include-wkt",
      "false",
    ],
  );
  assert_eq!(
    out.status, 0,
    "query {text:?} exited {}:\n{}",
    out.status, out.stderr
  );
  serde_json::from_str(&out.stdout).expect("the query must print json")
}

// the stage colours its verbs with ansi escapes; the words are asserted without them
fn plain(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  let mut rest = text;
  while let Some(start) = rest.find('\x1b') {
    out.push_str(&rest[..start]);
    rest = match rest[start..].find('m') {
      Some(end) => &rest[start + end + 1..],
      None => "",
    };
  }
  out.push_str(rest);
  out
}

fn level_counts(conn: &rusqlite::Connection) -> Vec<(u8, i64)> {
  conn
    .prepare(
      "SELECT admin_level, COUNT(*) FROM admin_levels GROUP BY admin_level ORDER BY admin_level",
    )
    .expect("failed to prepare")
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read a level count"))
    .collect()
}

// the ids of the rows at these levels, in order: equal strings mean the same rows
fn ids(conn: &rusqlite::Connection, levels: &str) -> String {
  conn
    .query_row(
      &format!(
        "SELECT COALESCE(GROUP_CONCAT(id, ','), '') \
         FROM (SELECT id FROM admin_levels WHERE admin_level IN ({levels}) ORDER BY id)"
      ),
      [],
      |r| r.get(0),
    )
    .expect("failed to list the ids")
}

fn ledger_count(conn: &rusqlite::Connection) -> Option<i64> {
  conn
    .query_row("SELECT admin_levels_count FROM osm_pbf_files", [], |r| {
      r.get(0)
    })
    .expect("failed to read the ledger")
}

fn hierarchy_rows(conn: &rusqlite::Connection) -> Vec<(i64, String, String)> {
  conn
    .prepare(
      "SELECT admin_level_id, json(ancestor_ids), user_friendly_name \
       FROM admin_levels_hierarchy ORDER BY admin_level_id",
    )
    .expect("failed to prepare")
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read a hierarchy row"))
    .collect()
}

fn hierarchy_count(s: &scratch) -> i64 {
  count(&s.ledger(), "SELECT COUNT(*) FROM admin_levels_hierarchy")
}

fn rtree_count(s: &scratch) -> i64 {
  count(&s.ledger(), "SELECT COUNT(*) FROM admin_levels_rtree")
}

// 00.00. the stage is incremental: a second run finds every candidate already processed
#[test]
#[ignore]
fn _00_00_re_running_the_stage_finds_nothing_remaining() {
  let w = world();
  let s = extracted(w, "admin_level_rerun", "2", &[]);
  admin_levels_at(w, &s.dir, "2,4,8", &[]);
  let before = ids(&s.ledger(), "2,4,8");
  assert_eq!(
    count(&s.ledger(), "SELECT COUNT(*) FROM admin_levels"),
    COUNTRY_STATE_CITY,
    "{REGENERATE}"
  );

  let again = plain(&admin_levels_at(w, &s.dir, "2,4,8", &[]).stdout);
  for line in [
    "1 found",
    "0 remaining",
    "skipping city — nothing to extract",
  ] {
    assert!(again.contains(line), "missing {line:?} in:\n{again}");
  }
  assert_eq!(
    ids(&s.ledger(), "2,4,8"),
    before,
    "a rerun must not change the rows"
  );
}

// 00.01. levels add up across runs, and the ledger counts what the table holds
#[test]
#[ignore]
fn _00_01_levels_add_up_across_runs_and_the_ledger_follows() {
  let w = world();
  let s = extracted(w, "admin_level_add_up", "2", &[]);

  admin_levels_at(w, &s.dir, "2,4", &[]);
  assert_eq!(
    level_counts(&s.ledger()),
    vec![(2, 1), (4, 27)],
    "{REGENERATE}"
  );
  assert_eq!(ledger_count(&s.ledger()), Some(28));
  let country_and_states = ids(&s.ledger(), "2,4");

  admin_levels_at(w, &s.dir, "8", &[]);
  assert_eq!(level_counts(&s.ledger()), vec![(2, 1), (4, 27), (8, 12)]);
  assert_eq!(ledger_count(&s.ledger()), Some(COUNTRY_STATE_CITY));
  assert_eq!(
    ids(&s.ledger(), "2,4"),
    country_and_states,
    "the levels of the first run must survive the second"
  );
}

// 00.02. --recreate starts over with the levels of that run only
#[test]
#[ignore]
fn _00_02_recreate_keeps_only_the_levels_of_the_run() {
  let w = world();
  let s = extracted(w, "admin_level_recreate", "2", &[]);
  admin_levels_at(w, &s.dir, "2,4,8", &[]);

  admin_levels_at(w, &s.dir, "8", &["--recreate"]);
  assert_eq!(level_counts(&s.ledger()), vec![(8, 12)], "{REGENERATE}");
  assert_eq!(ledger_count(&s.ledger()), Some(12));
}

// 00.03. --recreate takes the derived tables down with the areas
#[test]
#[ignore]
fn _00_03_recreate_empties_the_hierarchy_and_the_rtree() {
  let w = world();
  let s = extracted(w, "admin_level_recreate_derived", "2", &[]);
  admin_levels_at(w, &s.dir, "2,4,8", &[]);
  index_at(w, &s.dir, &[]);
  assert_eq!(hierarchy_count(&s), COUNTRY_STATE_CITY, "{REGENERATE}");
  assert_eq!(rtree_count(&s), COUNTRY_STATE_CITY);
  assert!(s.dir.join("database.tantivy").is_dir());

  admin_levels_at(w, &s.dir, "8", &["--recreate"]);
  assert_eq!(
    hierarchy_count(&s),
    0,
    "the hierarchy is derived from the areas"
  );
  assert_eq!(rtree_count(&s), 0, "the rtree is derived from the areas");
  assert_eq!(count(&s.ledger(), "SELECT COUNT(*) FROM admin_levels"), 12);
}

// 00.04. the stage names every level it runs by its place on the scale
#[test]
#[ignore]
fn _00_04_the_stage_names_every_level_of_the_scale() {
  let w = world();
  let s = extracted(w, "admin_level_names", "2", &[]);
  let out = plain(&admin_levels_at(w, &s.dir, "2,4,8", &[]).stdout);

  let headers = ["level 2 (country)", "level 4 (state)", "level 8 (city)"];
  let positions: Vec<usize> = headers
    .iter()
    .map(|h| {
      out
        .find(h)
        .unwrap_or_else(|| panic!("missing {h:?} in:\n{out}"))
    })
    .collect();
  assert!(
    positions.windows(2).all(|p| p[0] < p[1]),
    "the levels run in the order given"
  );
  for line in ["1 country in", "27 state in", "12 city in"] {
    assert!(
      out.contains(line),
      "missing {line:?} in:\n{out}\n{REGENERATE}"
    );
  }
}

// 00.05. a stored level outside the scale is a defence, not a path: the row is skipped aloud
#[test]
#[ignore]
fn _00_05_a_row_outside_the_scale_is_skipped_with_a_warning() {
  let w = world();
  let s = extracted(w, "admin_level_off_the_scale", "2", &[]);
  admin_levels_at(w, &s.dir, "2,4,8", &[]);
  {
    let conn = rusqlite::Connection::open(s.dir.join("database.sqlite3"))
      .expect("failed to open the scratch database for writing");
    let changed = conn
      .execute(
        "UPDATE admin_levels SET admin_level = 11 WHERE relation_id = ?1",
        [SANTOS_RELATION],
      )
      .expect("failed to write the level outside the scale");
    assert_eq!(changed, 1, "{REGENERATE}");
  }

  let out = index_at(w, &s.dir, &["admin-levels-hierarchy"]);
  let warning =
    format!("warn: admin_levels row {SANTOS_ID} carries level 11, outside the scale; skipped");
  assert!(out.stderr.contains(&warning), "stderr: {}", out.stderr);
  assert_eq!(
    hierarchy_count(&s),
    COUNTRY_STATE_CITY - 1,
    "every area but the skipped one gets a hierarchy row"
  );
}

// 01.00. the rtree is filled by its own stage, one box per row, and rebuilt from scratch each time
#[test]
#[ignore]
fn _01_00_index_coordinates_boxes_every_row_and_is_idempotent() {
  let w = world();
  let s = extracted(w, "admin_level_rtree", "2", &[]);
  admin_levels_at(w, &s.dir, "2,4,8", &[]);
  assert_eq!(rtree_count(&s), 0, "extraction leaves the rtree empty");

  index_at(w, &s.dir, &["coordinates"]);
  assert_eq!(rtree_count(&s), COUNTRY_STATE_CITY, "{REGENERATE}");
  index_at(w, &s.dir, &["coordinates"]);
  assert_eq!(
    rtree_count(&s),
    COUNTRY_STATE_CITY,
    "a rerun rebuilds the same boxes"
  );
}

// 02.00. peers resolve in parallel and streets in pages, and the result never depends on it
#[test]
#[ignore]
fn _02_00_the_hierarchy_stage_is_deterministic() {
  let w = world();
  let s = extracted(w, "admin_level_hierarchy_twice", "2", &[]);
  admin_levels_at(w, &s.dir, "2,4,8,10,12", &[]);

  index_at(w, &s.dir, &["admin-levels-hierarchy"]);
  let first_run = hierarchy_rows(&s.ledger());
  assert_eq!(first_run.len() as i64, EVERY_LEVEL, "{REGENERATE}");

  index_at(w, &s.dir, &["admin-levels-hierarchy"]);
  assert!(
    hierarchy_rows(&s.ledger()) == first_run,
    "the second run must write the same chains and labels"
  );
}

// 02.01. the hierarchy row, not the area, is the unit of search
#[test]
#[ignore]
fn _02_01_the_search_index_is_built_from_the_hierarchy_rows() {
  let w = world();
  let s = extracted(w, "admin_level_search_source", "2", &[]);
  admin_levels_at(w, &s.dir, "2,4,8", &[]);

  index_at(w, &s.dir, &["user-friendly-name"]);
  let empty = query_at(w, &s.dir, "santos");
  assert!(
    matches(&empty).is_empty(),
    "without hierarchy rows there is nothing to find, got {empty}"
  );

  index_at(w, &s.dir, &["admin-levels-hierarchy"]);
  index_at(w, &s.dir, &["user-friendly-name"]);
  let found = query_at(w, &s.dir, "santos");
  let top = first(&found);
  assert_eq!(levels_of(top).last(), Some(&8));
  assert_eq!(name_at(top, 8).as_deref(), Some("Santos"));
}
