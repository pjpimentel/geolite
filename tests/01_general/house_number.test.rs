use std::path::Path;

use crate::admin_level::{admin_levels_at, index_at};
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
const INJECTED_STREET: &str = "Rua Januário dos Santos";
const INJECTED_POINT: (f64, f64) = (-23.98202, -46.31005);
const FIRST_INJECTED_NODE: i64 = 9_000_000_001;

fn house_numbers_at(w: &world, dir: &Path, extra: &[&str]) -> output {
  let mut args = vec!["--preset", "brazil", "exec", "extract-osm-house-numbers"];
  args.extend_from_slice(extra);
  stage(w, dir, &args)
}

// the three stages of the file, then the streets, in a scratch of its own
fn streets(w: &world, name: &str) -> scratch {
  let s = extracted(w, name, "2", &[]);
  admin_levels_at(w, &s.dir, "12", &[]);
  s
}

// the streets and the house numbers of the fixture
fn linked(w: &world, name: &str) -> (scratch, output) {
  let s = streets(w, name);
  let out = house_numbers_at(w, &s.dir, &[]);
  (s, out)
}

// the streets and the house numbers of a scratch, indexed so that the scratch can be queried
fn indexed(w: &world, name: &str) -> scratch {
  let (s, _) = linked(w, name);
  for index in ["admin-levels-hierarchy", "user-friendly-name"] {
    index_at(w, &s.dir, index);
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

// the streets of the fixture, then one node per number on the numbered street, then the house
// numbers: the shapes the fixture never brings, through the real stage
fn injected(w: &world, name: &str, numbers: &[&str]) -> (scratch, output) {
  const SQL_INSERT_NODE: &str = "
    INSERT INTO osm_nodes (
      id,
      osm_pbf_chunk_id,
      payload
    ) VALUES (
      ?1,
      NULL,
      JSONB(?2)
    )
  ";

  let s = streets(w, name);
  {
    let conn = rusqlite::Connection::open(s.dir.join("database.osm_data.sqlite3"))
      .expect("failed to open the scratch osm data for writing");
    for (offset, number) in numbers.iter().enumerate() {
      let payload = json!({
        "lat": INJECTED_POINT.0,
        "lon": INJECTED_POINT.1,
        "tags": { "addr:housenumber": number, "addr:street": INJECTED_STREET }
      })
      .to_string();
      conn
        .execute(
          SQL_INSERT_NODE,
          rusqlite::params![FIRST_INJECTED_NODE + offset as i64, payload],
        )
        .expect("failed to insert the node");
    }
  }
  let out = house_numbers_at(w, &s.dir, &[]);
  (s, out)
}

fn injected_numbers(conn: &rusqlite::Connection, count: usize) -> Vec<(i64, String)> {
  const SQL_INJECTED_NUMBERS: &str = "
    SELECT node_id, number
    FROM house_numbers
    WHERE node_id >= ?1
      AND node_id < ?2
    ORDER BY node_id
  ";

  conn
    .prepare(SQL_INJECTED_NUMBERS)
    .expect("failed to prepare")
    .query_map(
      [FIRST_INJECTED_NODE, FIRST_INJECTED_NODE + count as i64],
      |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .expect("failed to query")
    .map(|r| r.expect("failed to read a house number"))
    .collect()
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

// 00.06. the rows land in node id order, whatever the machine's thread count
#[test]
#[ignore]
fn _00_06_the_rows_are_inserted_in_node_id_order() {
  let w = world();
  let (s, _) = linked(w, "house_number_order");
  let conn = s.ledger();
  let by_rowid: String = conn
    .query_row(
      "SELECT COALESCE(GROUP_CONCAT(node_id, ','), '') \
       FROM (SELECT node_id FROM house_numbers ORDER BY id)",
      [],
      |r| r.get(0),
    )
    .expect("failed to list the node ids by rowid");
  assert_eq!(by_rowid, node_ids(&conn), "{REGENERATE}");
}

// 00.07. a stage with no candidate at all reports zero and writes nothing
#[test]
#[ignore]
fn _00_07_a_stage_without_candidates_reports_zero() {
  let w = world();
  let s = extracted(
    w,
    "house_number_no_candidates",
    "2",
    &["--tags-ignore-list", "addr:housenumber"],
  );
  admin_levels_at(w, &s.dir, "12", &[]);
  let out = house_numbers_at(w, &s.dir, &[]);
  assert!(
    extracted_line(&out.stdout, 0),
    "a stage without candidates reports zero:\n{}",
    out.stdout
  );
  assert_eq!(count(&s.ledger(), "SELECT COUNT(*) FROM house_numbers"), 0);
}

// 00.08. the intermediary data is what the numbers are linked from: deleting it before the link
// is refused
#[test]
#[ignore]
fn _00_08_the_intermediary_data_cannot_be_deleted_before_the_house_numbers_are_linked() {
  let w = world();
  let s = extracted(w, "house_number_delete_before_link", "2", &[]);
  admin_levels_at(w, &s.dir, "2,4,8,10,12", &[]);
  index_at(w, &s.dir, "admin-levels-hierarchy");
  let out = w.geolite_in(&s.dir, &["exec", "optimize-delete-intermediary-data"]);
  assert_eq!(out.status, 1, "stderr: {}", out.stderr);
  assert!(
    out
      .stderr
      .contains("optimize-delete-intermediary-data requires extract-osm-house-numbers"),
    "stderr: {}",
    out.stderr
  );
  assert!(s.dir.join("database.osm_data.sqlite3").is_file());
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

// 01.02. the stage keeps, drops and canonizes the shapes the fixture never brings, as the policy
// of the preset says: brazil drops its non-values, the default keeps them
#[test]
#[ignore]
fn _01_02_the_stage_keeps_drops_and_canonizes_the_shapes_the_fixture_never_brings() {
  type shape = (&'static str, Option<&'static str>, Option<&'static str>);
  const SHAPES: [shape; 20] = [
    ("  100  ", Some("100"), Some("100")),
    ("12 a", Some("12A"), Some("12A")),
    ("12-a", Some("12A"), Some("12A")),
    ("12a", Some("12A"), Some("12A")),
    ("12A", Some("12A"), Some("12A")),
    ("12-14", Some("12-14"), Some("12-14")),
    ("Lote 5", Some("Lote 5"), Some("Lote 5")),
    ("82-52", Some("82-52"), Some("82-52")),
    ("25B-48", Some("25B-48"), Some("25B-48")),
    ("16i56", Some("16i56"), Some("16i56")),
    ("", None, None),
    ("   ", None, None),
    ("s/n", None, Some("s/n")),
    ("S/N", None, Some("S/N")),
    ("  s/n  ", None, Some("s/n")),
    ("Sn", None, Some("Sn")),
    ("s/nº", None, Some("s/nº")),
    ("s/no", None, Some("s/no")),
    ("s/n.", None, Some("s/n.")),
    ("s n", None, Some("s n")),
  ];
  let stored = |under: fn(&shape) -> Option<&'static str>| {
    SHAPES
      .iter()
      .enumerate()
      .filter_map(|(offset, shape)| {
        under(shape).map(|number| (FIRST_INJECTED_NODE + offset as i64, number.to_string()))
      })
      .collect::<Vec<(i64, String)>>()
  };

  let w = world();
  let raws: Vec<&str> = SHAPES.iter().map(|shape| shape.0).collect();
  let (s, out) = injected(w, "house_number_shapes", &raws);
  let under_brazil = stored(|shape| shape.1);
  assert!(
    extracted_line(&out.stdout, HOUSE_NUMBERS + under_brazil.len() as i64),
    "stdout: {}",
    out.stdout
  );
  assert_eq!(
    injected_numbers(&s.ledger(), SHAPES.len()),
    under_brazil,
    "under brazil"
  );

  let out = stage(
    w,
    &s.dir,
    &["exec", "extract-osm-house-numbers", "--recreate"],
  );
  let under_default = stored(|shape| shape.2);
  assert!(
    extracted_line(&out.stdout, HOUSE_NUMBERS + under_default.len() as i64),
    "stdout: {}",
    out.stdout
  );
  assert_eq!(
    injected_numbers(&s.ledger(), SHAPES.len()),
    under_default,
    "under the default preset, which drops no value"
  );
}

// 01.03. a compound number matches its stored value whatever the case of its letter and whether
// its parts are hyphenated, and only where the policy reads compounds
#[test]
#[ignore]
fn _01_03_a_compound_number_matches_across_its_letter_case_and_separator() {
  let w = world();
  let (s, _) = injected(w, "house_number_compound_keys", &["25B-48", "16i56"]);
  for index in ["admin-levels-hierarchy", "user-friendly-name"] {
    index_at(w, &s.dir, index);
  }

  for typed in ["25b-48", "16I56", "16i-56"] {
    let result = asked(w, &s.dir, "colombia", typed);
    assert_eq!(
      first(&result)["house_number"],
      json!({ "number": typed, "kind": "exact" }),
      "{typed}"
    );
    assert!(levels_of(first(&result)).contains(&30), "{typed}");
  }
  assert!(
    first(&asked(w, &s.dir, "brazil", "16I56"))
      .get("house_number")
      .is_none(),
    "brazil reads no number in '16I56'"
  );
}
