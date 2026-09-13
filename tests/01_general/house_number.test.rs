use std::path::Path;

use crate::common::harness::{decode_wkb, output, plain, query_at, world};
use crate::common::query::{first, levels_of};
use crate::extract::{REGENERATE, count, extracted, scratch, stage};
use crate::general::world;
use geo::{EuclideanDistance, Geometry};
use serde_json::{Value, json};

pub(crate) const HOUSE_NUMBERS: i64 = 473;
const BY_PROXIMITY: i64 = 33;
const BY_NAME: i64 = 440;
const SUFFIXED: &str = "40A";
const BRAZIL_DROPS: &str = "'s/n', 'sn', 's/nº', 's/no', 's/n.', 's n'";
const NUMBERED_STREET: &str = "rua januario dos santos, santos";
const NUMBERED_STREET_ID: i64 = 256_305_358;

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

// the streets and the house numbers of a scratch, indexed so that the scratch can be queried
fn indexed(w: &world, name: &str) -> scratch {
  let (s, _) = linked(w, name);
  for index in ["admin-levels-hierarchy", "user-friendly-name"] {
    stage(w, &s.dir, &["--preset", "brazil", "index", index]);
  }
  s
}

fn asked(w: &world, dir: &Path, preset: &str, number: &str) -> Value {
  query_at(w, dir, preset, &format!("{NUMBERED_STREET} {number}"))
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

// 00.04. every stored point is the candidate projected onto its street
#[test]
#[ignore]
fn _00_04_every_stored_point_lies_on_its_street() {
  let w = world();
  let (s, _) = linked(w, "house_number_geometry");
  let conn = s.ledger();
  let rows: Vec<(Vec<u8>, Vec<u8>, u8)> = conn
    .prepare(
      "SELECT h.wkb, a.wkb, a.admin_level FROM house_numbers h \
       JOIN admin_levels a ON a.id = h.admin_level_id",
    )
    .expect("failed to prepare")
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read a link"))
    .collect();
  assert_eq!(rows.len() as i64, HOUSE_NUMBERS, "{REGENERATE}");
  for (point, street, level) in rows {
    assert_eq!(level, 12, "a house number hangs from a street");
    let (Geometry::Point(point), Geometry::LineString(street)) =
      (decode_wkb(&point), decode_wkb(&street))
    else {
      panic!("a house number is a point and a street is a line");
    };
    assert!(
      point.euclidean_distance(&street) < 1e-9,
      "the stored point must lie on its street"
    );
  }
}

// 00.05. a link by name points at the street whose name the node carries
#[test]
#[ignore]
fn _00_05_a_link_by_name_names_the_street_the_node_names() {
  let w = world();
  let (s, _) = linked(w, "house_number_by_name");
  let conn = s.ledger();
  conn
    .execute(
      "ATTACH DATABASE ?1 AS osm_data",
      [s.dir
        .join("database.osm_data.sqlite3")
        .to_string_lossy()
        .as_ref()],
    )
    .expect("failed to attach the osm_data sibling");
  assert_eq!(
    count(
      &conn,
      "SELECT COUNT(*) FROM house_numbers WHERE strategy = 1"
    ),
    BY_NAME,
    "{REGENERATE}"
  );
  assert_eq!(
    count(
      &conn,
      "SELECT COUNT(*) FROM house_numbers h \
       JOIN admin_levels a ON a.id = h.admin_level_id \
       JOIN osm_data.osm_nodes n ON n.id = h.node_id \
       WHERE h.strategy = 1 AND LOWER(a.name) <> LOWER(n.payload->>'tags'->>'addr:street')"
    ),
    0,
    "a link by name must name the street the node names"
  );
}

// 01.00. the `#` prefix is a number only where the preset allows it
#[test]
#[ignore]
fn _01_00_the_hash_prefix_is_a_number_only_under_the_colombia_policy() {
  let w = world();
  for number in ["#197", "# 197"] {
    let result = asked(w, &w.data_path, "colombia", number);
    assert_eq!(
      first(&result)["house_number"],
      json!({ "number": "197", "kind": "exact" }),
      "{number}"
    );
    assert!(levels_of(first(&result)).contains(&30), "{number}");
  }
  let result = asked(w, &w.data_path, "brazil", "#197");
  assert!(
    first(&result).get("house_number").is_none(),
    "brazil reads no number in '#197'"
  );
  assert!(!levels_of(first(&result)).contains(&30));
}

// 01.01. a compound number matches its stored value under colombia and is never interpolated
#[test]
#[ignore]
fn _01_01_a_compound_number_is_read_only_where_the_policy_allows_it() {
  let w = world();
  let s = indexed(w, "house_number_compound");
  {
    let conn = rusqlite::Connection::open(s.dir.join("database.sqlite3"))
      .expect("failed to open the scratch database for writing");
    for (node_id, number, from) in [
      (9_000_000_001i64, "82-52", "197"),
      (9_000_000_002, "82-60", "232"),
    ] {
      let inserted = conn
        .execute(
          "INSERT INTO house_numbers (node_id, admin_level_id, number, wkb, strategy) \
           SELECT ?1, admin_level_id, ?2, wkb, 0 FROM house_numbers \
           WHERE admin_level_id = ?3 AND number = ?4",
          rusqlite::params![node_id, number, NUMBERED_STREET_ID, from],
        )
        .expect("failed to insert the compound number");
      assert_eq!(inserted, 1, "{REGENERATE}");
    }
  }

  let exact = asked(w, &s.dir, "colombia", "82-52");
  assert_eq!(
    first(&exact)["house_number"],
    json!({ "number": "82-52", "kind": "exact" })
  );
  let known = asked(w, &s.dir, "colombia", "197");
  assert_eq!(
    (&first(&exact)["latitude"], &first(&exact)["longitude"]),
    (&first(&known)["latitude"], &first(&known)["longitude"]),
    "the compound number sits on the point it was copied from"
  );
  assert_eq!(
    first(&asked(w, &s.dir, "colombia", "82-56"))["house_number"],
    json!({ "number": "82-56", "kind": "absent" }),
    "a compound number is never interpolated"
  );
  assert_eq!(
    first(&asked(w, &s.dir, "colombia", "82"))["house_number"]["kind"],
    "interpolated",
    "the control: a simple number interpolates on this street"
  );
  assert!(
    first(&asked(w, &s.dir, "brazil", "82-52"))
      .get("house_number")
      .is_none(),
    "brazil reads no number in '82-52'"
  );
}
