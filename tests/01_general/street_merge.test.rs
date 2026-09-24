use std::collections::HashMap;
use std::path::Path;

use crate::admin_level::{admin_levels_at, index_at, indexed};
use crate::common::harness::{decode_wkb, merged_way_ids_of, output, plain, query_at, world};
use crate::common::query::{level_at, matches, name_at, point_of};
use crate::extract::{REGENERATE, count, extracted, scratch, stage};
use crate::general::world;
use geo::{Contains, Geometry, LineString, Point};

pub(crate) const LOWER_WAY: u64 = 255_710_390;
pub(crate) const UPPER_WAY: u64 = 729_205_713;
const APART_WAYS: [u64; 2] = [169_924_327, 489_092_642];
const EUCLIDES_CROSSING: u64 = 38_791_238;
const EUCLIDES_WAYS: [u64; 5] = [
  EUCLIDES_CROSSING,
  483_127_426,
  483_127_429,
  858_402_097,
  858_402_098,
];
const EUCLIDES_ACROSS_THE_GAP: u64 = 482_330_396;
const SAO_PAULO: i64 = 596_409;
const GUARUJA: i64 = 596_927;
const EMBARE: i64 = 8_565_765;
const GONZAGA: i64 = 8_565_769;
const JOSE_MENINO: i64 = 8_565_771;
const STREETS: i64 = 12_878;
const STREETS_AFTER: i64 = 7_195;
const PIECES: i64 = 1_984;
pub(crate) const NUMBERS_MOVED: usize = 261;
const TEXT_QUERY: &str = "rua castro alves, embare, santos, sao paulo";
const EUCLIDES_GONZAGA_QUERY: &str = "rua euclides da cunha, gonzaga, santos, sao paulo";
const EUCLIDES_JOSE_MENINO_QUERY: &str = "rua euclides da cunha, jose menino, santos, sao paulo";

pub(crate) fn way(osm_id: u64) -> i64 {
  (osm_id << 1) as i64
}

fn writable(dir: &Path) -> rusqlite::Connection {
  rusqlite::Connection::open(dir.join("database.sqlite3"))
    .expect("failed to open the scratch database for writing")
}

fn rows_of(conn: &rusqlite::Connection, ways: &[u64]) -> i64 {
  let ids: Vec<String> = ways.iter().map(|&osm_id| way(osm_id).to_string()).collect();
  count(
    conn,
    &format!(
      "SELECT COUNT(*) FROM admin_levels WHERE id IN ({})",
      ids.join(", ")
    ),
  )
}

fn street_rows(conn: &rusqlite::Connection) -> i64 {
  count(
    conn,
    "SELECT COUNT(*) FROM admin_levels WHERE admin_level = 12",
  )
}

fn traced_rows(conn: &rusqlite::Connection) -> i64 {
  count(
    conn,
    "SELECT COUNT(*) FROM admin_levels WHERE merged_way_ids IS NOT NULL",
  )
}

fn assert_refused(out: &output, reason: &str) {
  assert_eq!(out.status, 1, "stderr: {}", out.stderr);
  assert!(plain(&out.stderr).contains(reason), "{}", out.stderr);
}

// the merge stage run where it is expected to refuse, so the exit code is the scenario's to assert
fn merge_refused(w: &world, dir: &Path) -> output {
  w.geolite_in(
    dir,
    &["--preset", "brazil", "exec", "optimize-merge-admin-levels"],
  )
}

pub(crate) fn merged_summary() -> String {
  format!(
    "merged {} street ways into {PIECES} streets in",
    STREETS - STREETS_AFTER + PIECES
  )
}

// a database as a build before the column left it. sqlite finds where the last column starts by
// walking back to a comma, so the ddl comment of merged_way_ids must carry none
pub(crate) fn drop_merged_way_ids(dir: &Path) {
  let conn = writable(dir);
  conn
    .execute_batch("ALTER TABLE admin_levels DROP COLUMN merged_way_ids")
    .expect("failed to drop merged_way_ids");
  assert_eq!(
    count(
      &conn,
      "SELECT COUNT(*) FROM PRAGMA_TABLE_INFO('admin_levels') WHERE name = 'merged_way_ids'"
    ),
    0
  );
}

fn merged_at(w: &world, dir: &Path) -> output {
  stage(
    w,
    dir,
    &["--preset", "brazil", "exec", "optimize-merge-admin-levels"],
  )
}

pub(crate) fn indexed_and_merged(w: &world, dir: &Path) {
  index_at(w, dir, "admin-levels-hierarchy");
  merged_at(w, dir);
  index_at(w, dir, "user-friendly-name");
  index_at(w, dir, "coordinates");
}

// the streets and the house numbers of a scratch, the hierarchy resolved and nothing merged yet
fn resolved(w: &world, name: &str) -> scratch {
  let s = extracted(w, name, "2", &[]);
  admin_levels_at(w, &s.dir, "2,4,8,10,12", &[]);
  stage(
    w,
    &s.dir,
    &["--preset", "brazil", "exec", "extract-osm-house-numbers"],
  );
  index_at(w, &s.dir, "admin-levels-hierarchy");
  s
}

fn lines_of(conn: &rusqlite::Connection, id: i64) -> Vec<LineString<f64>> {
  let wkb: Vec<u8> = conn
    .query_row("SELECT wkb FROM admin_levels WHERE id = ?1", [id], |r| {
      r.get(0)
    })
    .unwrap_or_else(|e| panic!("no admin_levels row {id}: {e}"));
  match decode_wkb(&wkb) {
    Geometry::LineString(line) => vec![line],
    Geometry::MultiLineString(lines) => lines.0,
    other => panic!("street {id} is not a line: {other:?}"),
  }
}

fn parents_of(conn: &rusqlite::Connection, id: i64) -> Vec<i64> {
  conn
    .prepare(
      "SELECT parent_id FROM admin_levels_hierarchy \
       WHERE admin_level_id = ?1 AND parent_id IS NOT NULL ORDER BY parent_id",
    )
    .expect("failed to prepare")
    .query_map([id], |r| r.get(0))
    .expect("failed to query")
    .map(|r| r.expect("failed to read a parent id"))
    .collect()
}

fn street_of_each_number(conn: &rusqlite::Connection) -> Vec<(i64, i64)> {
  conn
    .prepare("SELECT node_id, admin_level_id FROM house_numbers ORDER BY node_id")
    .expect("failed to prepare")
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read a house number"))
    .collect()
}

// what the merge may change, as three strings: the rows, their edges and the street of each number
fn snapshot(conn: &rusqlite::Connection) -> (String, String, String) {
  let read = |sql: &str| -> String {
    conn
      .query_row(sql, [], |r| r.get::<_, Option<String>>(0))
      .expect("failed to read the snapshot")
      .unwrap_or_default()
  };
  (
    read(
      "SELECT GROUP_CONCAT(id || ':' || LENGTH(wkb) || ':' || COALESCE(JSON(merged_way_ids), '')) FROM \
       (SELECT id, wkb, merged_way_ids FROM admin_levels ORDER BY id)",
    ),
    read(
      "SELECT GROUP_CONCAT(admin_level_id || '>' || COALESCE(parent_id, '')) FROM \
       (SELECT admin_level_id, parent_id FROM admin_levels_hierarchy ORDER BY admin_level_id, parent_id)",
    ),
    read(
      "SELECT GROUP_CONCAT(node_id || ':' || admin_level_id) FROM \
       (SELECT node_id, admin_level_id FROM house_numbers ORDER BY node_id)",
    ),
  )
}

fn set_post_codes(dir: &Path, codes: [(u64, Option<&str>); 2]) {
  let conn = writable(dir);
  for (osm_id, code) in codes {
    let changed = conn
      .execute(
        "UPDATE admin_levels SET post_code = ?2 WHERE id = ?1",
        rusqlite::params![way(osm_id), code],
      )
      .expect("failed to write the post code");
    assert_eq!(changed, 1, "{REGENERATE}");
  }
}

// 00.00. ways that touch fold into the one with the smallest id, keeping the lines of both
#[test]
#[ignore]
fn _00_00_ways_that_touch_fold_into_the_smallest_id() {
  let w = world();
  let s = resolved(w, "street_merge_fold");
  let (lower, upper) = {
    let conn = s.ledger();
    (
      lines_of(&conn, way(LOWER_WAY)),
      lines_of(&conn, way(UPPER_WAY)),
    )
  };

  let stdout = plain(&merged_at(w, &s.dir).stdout);
  assert!(
    stdout.contains(&merged_summary()),
    "{REGENERATE}:\n{stdout}"
  );

  let conn = s.ledger();
  assert_eq!(
    count(
      &conn,
      &format!(
        "SELECT COUNT(*) FROM admin_levels WHERE id = {}",
        way(UPPER_WAY)
      )
    ),
    0,
    "the way with the larger id is absorbed"
  );
  let survivor_way: i64 = conn
    .query_row(
      "SELECT way_id FROM admin_levels WHERE id = ?1",
      [way(LOWER_WAY)],
      |r| r.get(0),
    )
    .expect("failed to read the way of the street");
  assert_eq!(survivor_way, LOWER_WAY as i64);
  assert_eq!(
    lines_of(&conn, way(LOWER_WAY)),
    [lower, upper].concat(),
    "the street keeps the lines of both ways, the smallest id first"
  );
  assert_eq!(
    merged_way_ids_of(&conn, way(LOWER_WAY)),
    Some(vec![LOWER_WAY, UPPER_WAY]),
    "each line is named by the way it came from, in the order of the lines"
  );
  assert_eq!(
    count(
      &conn,
      "SELECT COUNT(*) FROM admin_levels WHERE admin_level = 12"
    ),
    STREETS_AFTER,
    "{REGENERATE}"
  );
  assert_eq!(
    count(
      &conn,
      "SELECT COUNT(*) FROM admin_levels WHERE merged_way_ids IS NOT NULL"
    ),
    PIECES,
    "every folded street is traced, and nothing else is"
  );
  assert_eq!(
    count(
      &conn,
      "SELECT COUNT(*) FROM admin_levels WHERE merged_way_ids IS NOT NULL AND \
       (TYPEOF(merged_way_ids) != 'blob' OR JSON_ARRAY_LENGTH(merged_way_ids) < 2)"
    ),
    0,
    "the trace is a jsonb array of two ways at least"
  );
}

// 00.01. ways farther apart than the reach stay apart, under the same name and the same chain
#[test]
#[ignore]
fn _00_01_ways_farther_apart_than_the_reach_stay_apart() {
  let w = world();
  let s = resolved(w, "street_merge_apart");
  let before: Vec<Vec<LineString<f64>>> = APART_WAYS
    .iter()
    .map(|&osm_id| lines_of(&s.ledger(), way(osm_id)))
    .collect();

  merged_at(w, &s.dir);

  let conn = s.ledger();
  for (osm_id, lines) in APART_WAYS.into_iter().zip(before) {
    assert_eq!(
      lines_of(&conn, way(osm_id)),
      lines,
      "way {osm_id} keeps its row; {REGENERATE}"
    );
    assert_eq!(
      merged_way_ids_of(&conn, way(osm_id)),
      None,
      "a street that was never folded is not traced"
    );
  }
}

// 00.02. the same name under another chain stays apart
#[test]
#[ignore]
fn _00_02_the_same_name_under_another_chain_stays_apart() {
  let w = world();
  let s = resolved(w, "street_merge_chains");
  merged_at(w, &s.dir);

  let per_parent: Vec<(i64, i64)> = s
    .ledger()
    .prepare(
      "SELECT h.parent_id, COUNT(*) FROM admin_levels_hierarchy h \
       JOIN admin_levels a ON a.id = h.admin_level_id \
       WHERE a.name = 'Rua Castro Alves' GROUP BY h.parent_id ORDER BY h.parent_id",
    )
    .expect("failed to prepare")
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read a count"))
    .collect();
  assert_eq!(
    per_parent,
    vec![(SAO_PAULO, 2), (GUARUJA, 1), (EMBARE, 1)],
    "{REGENERATE}"
  );
}

// 00.03. the chain of a folded street is the chain its ways had, and the tables stay consistent
#[test]
#[ignore]
fn _00_03_a_folded_street_keeps_the_chain_of_its_ways() {
  let w = world();
  let s = resolved(w, "street_merge_chain_kept");
  let (lower, upper) = {
    let conn = s.ledger();
    (
      parents_of(&conn, way(LOWER_WAY)),
      parents_of(&conn, way(UPPER_WAY)),
    )
  };
  assert_eq!(lower, upper, "the two ways share their chain; {REGENERATE}");

  merged_at(w, &s.dir);

  let conn = s.ledger();
  assert_eq!(parents_of(&conn, way(LOWER_WAY)), lower);
  assert_eq!(
    count(&conn, "SELECT COUNT(*) FROM admin_levels"),
    count(
      &conn,
      "SELECT COUNT(DISTINCT admin_level_id) FROM admin_levels_hierarchy"
    ),
    "every area keeps one hierarchy row at least, and no area that is gone keeps one"
  );
  assert_eq!(
    count(&conn, "SELECT COUNT(*) FROM pragma_foreign_key_check"),
    0,
    "no edge and no number may point at a row that is gone"
  );
}

// 00.04. the numbers of an absorbed way move to the street it was folded into
#[test]
#[ignore]
fn _00_04_house_numbers_follow_the_street_they_were_folded_into() {
  let w = world();
  let s = resolved(w, "street_merge_numbers");
  let (before, names): (Vec<(i64, i64)>, HashMap<i64, String>) = {
    let conn = s.ledger();
    let names = conn
      .prepare("SELECT id, name FROM admin_levels WHERE admin_level = 12")
      .expect("failed to prepare")
      .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
      .expect("failed to query")
      .map(|r| r.expect("failed to read a name"))
      .collect();
    (street_of_each_number(&conn), names)
  };

  merged_at(w, &s.dir);

  let conn = s.ledger();
  let after = street_of_each_number(&conn);
  assert_eq!(after.len(), before.len(), "no number is lost");
  let moved: Vec<(i64, i64)> = before
    .iter()
    .zip(&after)
    .filter(|(old, new)| old.1 != new.1)
    .map(|(old, new)| (old.1, new.1))
    .collect();
  assert_eq!(moved.len(), NUMBERS_MOVED, "{REGENERATE}");
  for (from, to) in moved {
    assert_eq!(
      names[&from], names[&to],
      "a number stays on a street of its name"
    );
  }
  assert_eq!(
    count(
      &conn,
      &format!(
        "SELECT COUNT(*) FROM house_numbers WHERE number = '35' AND admin_level_id = {}",
        way(LOWER_WAY)
      )
    ),
    1,
    "number 35 was on the way that was absorbed"
  );
}

// 00.05. a second run finds nothing to fold and leaves the indexes as they are
#[test]
#[ignore]
fn _00_05_a_second_run_changes_nothing_and_keeps_the_indexes() {
  let w = world();
  let s = resolved(w, "street_merge_twice");
  merged_at(w, &s.dir);
  index_at(w, &s.dir, "user-friendly-name");
  index_at(w, &s.dir, "coordinates");
  let rtree = count(&s.ledger(), "SELECT COUNT(*) FROM admin_levels_rtree");
  let before = snapshot(&s.ledger());

  let again = plain(&merged_at(w, &s.dir).stdout);

  assert!(
    again.contains("skipping merge-admin-levels — no street to merge"),
    "{again}"
  );
  assert_eq!(snapshot(&s.ledger()), before);
  assert!(s.dir.join("database.tantivy").is_dir());
  assert_eq!(
    count(&s.ledger(), "SELECT COUNT(*) FROM admin_levels_rtree"),
    rtree
  );
}

// 00.06. what it folds makes the two indexes after it stale: it clears them and says so
#[test]
#[ignore]
fn _00_06_the_indexes_after_it_are_cleared_and_have_to_be_recreated() {
  let w = world();
  let s = resolved(w, "street_merge_clears");
  index_at(w, &s.dir, "user-friendly-name");
  index_at(w, &s.dir, "coordinates");
  assert!(s.dir.join("database.tantivy").is_dir());

  let out = merged_at(w, &s.dir);

  assert!(
    plain(&out.stdout).contains(
      "next run `geolite exec index-user-friendly-name` and `geolite exec index-coordinates`"
    ),
    "{}",
    out.stdout
  );
  assert!(!s.dir.join("database.tantivy").exists());
  assert_eq!(
    count(&s.ledger(), "SELECT COUNT(*) FROM admin_levels_rtree"),
    0
  );
  let asked = w.geolite_in(
    &s.dir,
    &[
      "--preset",
      "brazil",
      "query",
      TEXT_QUERY,
      "--include-wkt",
      "false",
    ],
  );
  assert_eq!(asked.status, 1);
  assert!(
    asked.stderr.contains("tantivy index not found"),
    "{}",
    asked.stderr
  );

  index_at(w, &s.dir, "user-friendly-name");
  index_at(w, &s.dir, "coordinates");
  let result = query_at(w, &s.dir, "brazil", TEXT_QUERY);
  assert_eq!(matches(&result).len(), 1, "the street answers once");
}

// 00.07. the ledger counts what the table holds after the fold
#[test]
#[ignore]
fn _00_07_the_ledger_follows_the_table_after_the_merge() {
  let w = world();
  let s = resolved(w, "street_merge_ledger");
  merged_at(w, &s.dir);

  let conn = s.ledger();
  let (admins, houses): (i64, i64) = conn
    .query_row(
      "SELECT admin_levels_count, house_numbers_count FROM osm_pbf_files",
      [],
      |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .expect("failed to read the ledger");
  assert_eq!(
    admins,
    count(
      &conn,
      "SELECT COUNT(*) FROM admin_levels WHERE wkb IS NOT NULL"
    )
  );
  assert_eq!(houses, count(&conn, "SELECT COUNT(*) FROM house_numbers"));
}

// 00.08. resolving the hierarchy again over the folded rows writes the same edges
#[test]
#[ignore]
fn _00_08_resolving_the_hierarchy_again_keeps_the_edges_of_the_folded_streets() {
  let w = world();
  let s = resolved(w, "street_merge_rehierarchy");
  merged_at(w, &s.dir);
  let before = snapshot(&s.ledger());

  index_at(w, &s.dir, "admin-levels-hierarchy");

  assert_eq!(snapshot(&s.ledger()), before);
  let again = plain(&merged_at(w, &s.dir).stdout);
  assert!(again.contains("skipping merge-admin-levels"), "{again}");
}

// 00.09. a way extracted again after the fold comes back as a row, and the next run absorbs it again
#[test]
#[ignore]
fn _00_09_a_way_extracted_again_is_absorbed_again() {
  let w = world();
  let s = resolved(w, "street_merge_reextracted");
  merged_at(w, &s.dir);
  let before = snapshot(&s.ledger());
  let lines_before = lines_of(&s.ledger(), way(LOWER_WAY));

  admin_levels_at(w, &s.dir, "12", &[]);
  assert_eq!(
    count(
      &s.ledger(),
      "SELECT COUNT(*) FROM admin_levels WHERE admin_level = 12"
    ),
    STREETS,
    "every absorbed way is a row again"
  );

  index_at(w, &s.dir, "admin-levels-hierarchy");
  merged_at(w, &s.dir);

  assert_eq!(snapshot(&s.ledger()), before);
  assert_eq!(
    lines_of(&s.ledger(), way(LOWER_WAY)),
    lines_before,
    "a way the street already holds is not folded into it again"
  );
}

// 00.10. ways that share a neighbourhood fold into one street that belongs to every neighbourhood
// its ways were in, so each label answers once
#[test]
#[ignore]
fn _00_10_ways_that_share_a_neighbourhood_fold_into_a_street_that_belongs_to_all_of_them() {
  let w = world();
  let s = resolved(w, "street_merge_labels");
  assert_eq!(
    parents_of(&s.ledger(), way(EUCLIDES_CROSSING)),
    vec![GONZAGA],
    "the way that carries the street in Gonzaga has only that parent; {REGENERATE}"
  );

  indexed_and_merged(w, &s.dir);

  assert_eq!(
    parents_of(&s.ledger(), way(EUCLIDES_CROSSING)),
    vec![GONZAGA, JOSE_MENINO],
    "the street belongs to every neighbourhood its ways were in"
  );
  let result = query_at(w, &s.dir, "brazil", EUCLIDES_GONZAGA_QUERY);
  let in_gonzaga = matches(&result)
    .iter()
    .filter(|m| name_at(m, 12).as_deref() == Some("Rua Euclides da Cunha"))
    .filter(|m| name_at(m, 10).as_deref() == Some("Gonzaga"))
    .count();
  assert_eq!(in_gonzaga, 1, "the label answers once");
}

// 00.11. a gap wider than the reach keeps two pieces: the two ends of the street across an avenue
// with a median are 48 m apart
#[test]
#[ignore]
fn _00_11_a_gap_wider_than_the_reach_keeps_two_pieces() {
  let w = world();
  let s = resolved(w, "street_merge_gap");

  merged_at(w, &s.dir);

  let conn = s.ledger();
  assert_eq!(
    count(
      &conn,
      "SELECT COUNT(*) FROM admin_levels WHERE name = 'Rua Euclides da Cunha'"
    ),
    2,
    "{REGENERATE}"
  );
  for osm_id in [EUCLIDES_CROSSING, EUCLIDES_ACROSS_THE_GAP] {
    assert_eq!(
      count(
        &conn,
        &format!(
          "SELECT COUNT(*) FROM admin_levels WHERE id = {}",
          way(osm_id)
        )
      ),
      1,
      "way {osm_id} survives as the street of its piece"
    );
  }
}

// 00.12. each label of a street across two neighbourhoods answers a point inside its own
#[test]
#[ignore]
fn _00_12_each_label_of_a_street_answers_a_point_inside_its_own_neighbourhood() {
  let w = world();
  let s = resolved(w, "street_merge_label_points");
  indexed_and_merged(w, &s.dir);
  let conn = s.ledger();

  for (query, neighbourhood) in [
    (EUCLIDES_GONZAGA_QUERY, GONZAGA),
    (EUCLIDES_JOSE_MENINO_QUERY, JOSE_MENINO),
  ] {
    let result = query_at(w, &s.dir, "brazil", query);
    let across = matches(&result)
      .iter()
      .find(|m| {
        level_at(m, 12).and_then(|street| street["osm_way_id"].as_u64()) == Some(EUCLIDES_CROSSING)
      })
      .unwrap_or_else(|| panic!("the street that crosses both answers {query:?}"));
    let (latitude, longitude) = point_of(across);
    let blob: Vec<u8> = conn
      .query_row(
        "SELECT wkb FROM admin_levels WHERE id = ?1",
        [neighbourhood],
        |r| r.get(0),
      )
      .expect("failed to read the neighbourhood");
    assert!(
      decode_wkb(&blob).contains(&Point::new(longitude, latitude)),
      "{query:?} answers outside its neighbourhood"
    );
  }
}

// 00.13. a street of many ways keeps the line of every way, its own first and the others in id
// order, and its trace names them in that order
#[test]
#[ignore]
fn _00_13_a_street_of_many_ways_keeps_every_line_in_the_order_of_its_trace() {
  let w = world();
  let s = resolved(w, "street_merge_many_ways");
  let before: Vec<LineString<f64>> = {
    let conn = s.ledger();
    EUCLIDES_WAYS
      .iter()
      .flat_map(|&osm_id| lines_of(&conn, way(osm_id)))
      .collect()
  };

  merged_at(w, &s.dir);

  let conn = s.ledger();
  assert_eq!(
    merged_way_ids_of(&conn, way(EUCLIDES_CROSSING)),
    Some(EUCLIDES_WAYS.to_vec()),
    "{REGENERATE}"
  );
  assert_eq!(
    lines_of(&conn, way(EUCLIDES_CROSSING)),
    before,
    "one line per way, as the way has it, in the order of the trace"
  );
  assert_eq!(
    rows_of(&conn, &EUCLIDES_WAYS[1..]),
    0,
    "the other ways are absorbed"
  );
}

// 00.14. the three index stages resolve and index and fold nothing: only build, merge and the
// stage itself fold
#[test]
#[ignore]
fn _00_14_the_index_stages_alone_do_not_fold() {
  let w = world();
  let s = extracted(w, "street_merge_index_alone", "2", &[]);
  admin_levels_at(w, &s.dir, "2,4,8,10,12", &[]);

  let stdout = plain(&indexed(w, &s.dir));

  assert!(
    !stdout.contains("merge-admin-levels") && !stdout.contains("street ways into"),
    "{stdout}"
  );
  let conn = s.ledger();
  assert_eq!(street_rows(&conn), STREETS, "{REGENERATE}");
  assert_eq!(traced_rows(&conn), 0);
}

// 01.00. two different post codes are two streets
#[test]
#[ignore]
fn _01_00_two_different_post_codes_keep_two_streets() {
  let w = world();
  let s = resolved(w, "street_merge_post_codes_differ");
  set_post_codes(
    &s.dir,
    [
      (LOWER_WAY, Some("11000-000")),
      (UPPER_WAY, Some("11000-001")),
    ],
  );

  merged_at(w, &s.dir);

  let conn = s.ledger();
  for osm_id in [LOWER_WAY, UPPER_WAY] {
    assert_eq!(
      count(
        &conn,
        &format!(
          "SELECT COUNT(*) FROM admin_levels WHERE id = {}",
          way(osm_id)
        )
      ),
      1,
      "way {osm_id} keeps its row"
    );
  }
}

// 01.01. a missing post code does not split a street, and the code that exists survives
#[test]
#[ignore]
fn _01_01_a_missing_post_code_does_not_split_a_street() {
  let w = world();
  let s = resolved(w, "street_merge_post_code_missing");
  set_post_codes(&s.dir, [(LOWER_WAY, None), (UPPER_WAY, Some("11000-001"))]);

  merged_at(w, &s.dir);

  let conn = s.ledger();
  let code: Option<String> = conn
    .query_row(
      "SELECT post_code FROM admin_levels WHERE id = ?1",
      [way(LOWER_WAY)],
      |r| r.get(0),
    )
    .expect("failed to read the post code of the street");
  assert_eq!(
    code.as_deref(),
    Some("11000-001"),
    "the street takes the code of the way that had one"
  );
  assert_eq!(
    count(
      &conn,
      &format!(
        "SELECT COUNT(*) FROM admin_levels WHERE id = {}",
        way(UPPER_WAY)
      )
    ),
    0
  );
}

// 02.00. it needs a resolved hierarchy: without one it says so and changes nothing
#[test]
#[ignore]
fn _02_00_it_refuses_a_database_without_a_hierarchy() {
  let w = world();
  let s = extracted(w, "street_merge_no_hierarchy", "2", &[]);
  admin_levels_at(w, &s.dir, "2,4,8,10,12", &[]);

  let out = merge_refused(w, &s.dir);

  assert_refused(
    &out,
    "optimize-merge-admin-levels requires index-admin-levels-hierarchy",
  );
  assert_eq!(
    count(
      &s.ledger(),
      "SELECT COUNT(*) FROM admin_levels WHERE admin_level = 12"
    ),
    STREETS,
    "{REGENERATE}"
  );
}

// 02.01. it needs rows to fold: an empty database has no hierarchy to require
#[test]
#[ignore]
fn _02_01_it_refuses_an_empty_database() {
  let w = world();
  let s = extracted(w, "street_merge_empty", "2", &[]);

  let out = merge_refused(w, &s.dir);

  assert_refused(
    &out,
    "optimize-merge-admin-levels requires index-admin-levels-hierarchy",
  );
}

// 02.02. it needs the hierarchy to cover every row: the ways extracted again after a fold have no
// edge yet, and it refuses until the hierarchy is resolved again
#[test]
#[ignore]
fn _02_02_it_refuses_a_hierarchy_that_no_longer_covers_every_row() {
  let w = world();
  let s = resolved(w, "street_merge_stale_hierarchy");
  merged_at(w, &s.dir);
  admin_levels_at(w, &s.dir, "12", &[]);
  let before = snapshot(&s.ledger());

  let out = merge_refused(w, &s.dir);

  assert_refused(
    &out,
    "optimize-merge-admin-levels requires index-admin-levels-hierarchy",
  );
  assert_eq!(
    snapshot(&s.ledger()),
    before,
    "a refused run writes nothing"
  );
  assert_eq!(street_rows(&s.ledger()), STREETS, "{REGENERATE}");
}

// 02.03. a way whose geometry cannot be read warns and is left alone, and the rest still fold: the
// blob never reaches the real pipeline
#[test]
#[ignore]
fn _02_03_a_way_with_an_unreadable_geometry_is_left_alone_and_the_rest_fold() {
  let w = world();
  let s = resolved(w, "street_merge_unreadable_geometry");
  let shortened = writable(&s.dir)
    .execute(
      "UPDATE admin_levels SET wkb = x'0001' WHERE id = ?1",
      [way(UPPER_WAY)],
    )
    .expect("failed to shorten the blob");
  assert_eq!(shortened, 1, "{REGENERATE}");

  let out = merged_at(w, &s.dir);

  assert!(out.stderr.contains("blob too short"), "{}", out.stderr);
  let conn = s.ledger();
  assert_eq!(
    merged_way_ids_of(&conn, way(LOWER_WAY)),
    None,
    "its only neighbour has no line to touch"
  );
  assert_eq!(
    rows_of(&conn, &[UPPER_WAY]),
    1,
    "the way that cannot be read keeps its row"
  );
  assert_eq!(traced_rows(&conn), PIECES - 1, "{REGENERATE}");
}

// 03.00. a database built before the column gains it on the next write command, under the same
// schema version, and the fold traces its streets there
#[test]
#[ignore]
fn _03_00_a_database_without_merged_way_ids_gains_it_on_the_next_write_command() {
  let w = world();
  let s = resolved(w, "street_merge_column_added");
  drop_merged_way_ids(&s.dir);
  let version = count(&s.ledger(), "PRAGMA user_version");

  merged_at(w, &s.dir);

  let conn = s.ledger();
  assert_eq!(
    merged_way_ids_of(&conn, way(LOWER_WAY)),
    Some(vec![LOWER_WAY, UPPER_WAY])
  );
  assert_eq!(
    count(&conn, "PRAGMA user_version"),
    version,
    "no version bump for an added column"
  );
}

// 03.01. the answer names the ways of a folded street and of nothing else, and a database that
// never got the column still answers, without them
#[test]
#[ignore]
fn _03_01_query_still_answers_on_a_database_without_merged_way_ids() {
  let w = world();
  let s = resolved(w, "street_merge_column_absent");
  indexed_and_merged(w, &s.dir);
  let trace_at = |level: u64| -> Option<serde_json::Value> {
    let result = query_at(w, &s.dir, "brazil", TEXT_QUERY);
    level_at(&matches(&result)[0], level)
      .unwrap_or_else(|| panic!("the top match has no level {level}; {REGENERATE}"))
      .get("osm_merged_way_ids")
      .cloned()
  };
  assert_eq!(
    trace_at(12),
    Some(serde_json::json!([LOWER_WAY, UPPER_WAY]))
  );
  assert_eq!(
    trace_at(10),
    None,
    "an area that is not a fold carries no key"
  );

  drop_merged_way_ids(&s.dir);

  assert_eq!(trace_at(12), None);
}
