use std::path::Path;

use crate::common::harness::{output, plain, world};
use crate::extract::{REGENERATE, count, extracted, scratch, stage};
use crate::general::world;

const HOUSE_NUMBERS: i64 = 473;
const BY_PROXIMITY: i64 = 33;
const BY_NAME: i64 = 440;
const SUFFIXED: &str = "40A";
const BRAZIL_DROPS: &str = "'s/n', 'sn', 's/nº', 's/no', 's/n.', 's n'";

fn house_numbers_at(w: &world, dir: &Path, extra: &[&str]) -> output {
  let mut args = vec!["--preset", "brazil", "extract", "osm-house-numbers"];
  args.extend_from_slice(extra);
  stage(w, dir, &args)
}

// the three stages of the file, then the streets and the house numbers, in a scratch of its own
fn linked(w: &world, name: &str) -> (scratch, output) {
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
      "12",
    ],
  );
  let out = house_numbers_at(w, &s.dir, &[]);
  (s, out)
}

fn extracted_line(stdout: &str, inserted: i64) -> bool {
  plain(stdout).contains(&format!("extracted {inserted} house numbers in"))
}

// the linked node ids, in order: equal strings mean the same links
fn node_ids(conn: &rusqlite::Connection) -> String {
  conn
    .query_row(
      "SELECT COALESCE(GROUP_CONCAT(node_id, ','), '') \
       FROM (SELECT node_id FROM house_numbers ORDER BY node_id)",
      [],
      |r| r.get(0),
    )
    .expect("failed to list the node ids")
}

fn strategy_counts(conn: &rusqlite::Connection) -> Vec<(u8, i64)> {
  conn
    .prepare("SELECT strategy, COUNT(*) FROM house_numbers GROUP BY strategy ORDER BY strategy")
    .expect("failed to prepare")
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read a strategy count"))
    .collect()
}

fn ledger_count(conn: &rusqlite::Connection) -> Option<i64> {
  conn
    .query_row("SELECT house_numbers_count FROM osm_pbf_files", [], |r| {
      r.get(0)
    })
    .expect("failed to read the ledger")
}

fn assert_full_table(s: &scratch) {
  let conn = s.ledger();
  assert_eq!(
    count(&conn, "SELECT COUNT(*) FROM house_numbers"),
    HOUSE_NUMBERS,
    "{REGENERATE}"
  );
  assert_eq!(
    ledger_count(&conn),
    Some(HOUSE_NUMBERS),
    "the ledger must follow the table"
  );
}

// 00.00. the stage links every candidate it can and names the count it stored
#[test]
#[ignore]
fn _00_00_the_stage_links_every_candidate_it_can_and_names_the_count() {
  let w = world();
  let (s, out) = linked(w, "house_number_stage");
  assert!(
    extracted_line(&out.stdout, HOUSE_NUMBERS),
    "{REGENERATE}:\n{}",
    out.stdout
  );
  assert_full_table(&s);
  let conn = s.ledger();
  assert_eq!(
    strategy_counts(&conn),
    vec![(0, BY_PROXIMITY), (1, BY_NAME)],
    "{REGENERATE}"
  );
  assert_eq!(
    count(
      &conn,
      "SELECT COUNT(*) FROM house_numbers h \
       LEFT JOIN admin_levels a ON a.id = h.admin_level_id WHERE a.id IS NULL"
    ),
    0,
    "every house number must point at a street row"
  );
}

// 00.01. a rerun finds every node already linked: nothing is inserted, nothing changes
#[test]
#[ignore]
fn _00_01_re_running_the_stage_inserts_nothing_and_keeps_the_rows() {
  let w = world();
  let (s, _) = linked(w, "house_number_rerun");
  let before = node_ids(&s.ledger());
  let again = house_numbers_at(w, &s.dir, &[]);
  assert!(
    extracted_line(&again.stdout, 0),
    "a rerun inserts nothing:\n{}",
    again.stdout
  );
  assert_eq!(
    node_ids(&s.ledger()),
    before,
    "a rerun must not change the rows"
  );
  assert_full_table(&s);
}

// 00.02. --recreate empties the table and links the same nodes again
#[test]
#[ignore]
fn _00_02_recreate_rebuilds_the_same_links() {
  let w = world();
  let (s, _) = linked(w, "house_number_recreate");
  let before = node_ids(&s.ledger());
  let rebuilt = house_numbers_at(w, &s.dir, &["--recreate"]);
  assert!(
    extracted_line(&rebuilt.stdout, HOUSE_NUMBERS),
    "recreate starts from an empty table:\n{}",
    rebuilt.stdout
  );
  assert_eq!(
    node_ids(&s.ledger()),
    before,
    "recreate must link the same nodes"
  );
  assert_full_table(&s);
}

// 00.03. what is stored is the canonical form: trimmed, suffix upper case, no non-values
#[test]
#[ignore]
fn _00_03_stored_numbers_are_canonical() {
  let w = world();
  let (s, _) = linked(w, "house_number_canonical");
  let conn = s.ledger();
  let non_values = format!("LOWER(number) IN ({BRAZIL_DROPS})");
  let stored: [(&str, &str); 4] = [
    ("an untrimmed number", "number <> TRIM(number)"),
    ("an empty number", "number = ''"),
    (
      "a lower-case suffix",
      "number GLOB '[0-9]*[a-z]' AND number NOT GLOB '*[^0-9a-z]*'",
    ),
    ("a non-value", &non_values),
  ];
  for (what, predicate) in stored {
    assert_eq!(
      count(
        &conn,
        &format!("SELECT COUNT(*) FROM house_numbers WHERE {predicate}")
      ),
      0,
      "{what} was stored"
    );
  }
  assert_eq!(
    count(
      &conn,
      &format!("SELECT COUNT(*) FROM house_numbers WHERE number = '{SUFFIXED}'")
    ),
    1,
    "{REGENERATE}"
  );
}
