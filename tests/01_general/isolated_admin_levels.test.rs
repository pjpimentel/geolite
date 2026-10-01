use std::path::{Path, PathBuf};

use crate::admin_level::admin_levels_at;
use crate::admin_level_hierarchy::{area, relation, resolved_at, street};
use crate::common::harness::{assert_in_order, open_sqlite_at, output, plain, query_at, world};
use crate::common::query::matches;
use crate::extract::{REGENERATE, count, extracted, scratch, stage};
use crate::general::world;
use crate::street_merge::{
  TEXT_QUERY, UPPER_WAY, assert_a_run_with_nothing_to_do_keeps_the_indexes, assert_refused,
  assert_the_indexes_are_cleared_and_recreated, assert_the_ledger_follows_the_tables,
  assert_the_tables_are_consistent, parents_of, resolved, snapshot, way, writable,
};

const STAGE: &str = "optimize-delete-isolated-admin-levels";
const MERGE_STAGE: &str = "optimize-merge-admin-levels";
pub(crate) const NOTHING_ISOLATED: &str = "skipping delete-isolated-admin-levels — no country without children and no street without parents";
pub(crate) const ONE_OF_EACH_DELETED: &str =
  "deleted 1 countries without children and 1 streets without parents in";

const CLEARING: &str = "clearing addresses and coordinates... done";
const NEXT_RUN: &str = "next run `geolite exec index-addresses`";

const COUNTRY: u64 = 1;
const NEIGHBOUR: u64 = 2;
const CITY: u64 = 3;
const STATE: u64 = 4;
const NEIGHBOURHOOD: u64 = 5;
const INSIDE_STREET: u64 = 10;
const OUTSIDE_STREET: u64 = 11;
const FAR_COUNTRY: u64 = 6;
const FAR_STREET: u64 = 12;

pub(crate) fn isolated_deleted_at(w: &world, dir: &Path) -> output {
  stage(w, dir, &["--preset", "brazil", "exec", STAGE])
}

fn ledger(dir: &Path) -> rusqlite::Connection {
  open_sqlite_at(&dir.join("database.sqlite3"))
}

fn holds(conn: &rusqlite::Connection, id: i64) -> bool {
  count(
    conn,
    &format!("SELECT COUNT(*) FROM admin_levels WHERE id = {id}"),
  ) == 1
}

pub(crate) fn holds_what_was_isolated(conn: &rusqlite::Connection) -> bool {
  holds(conn, relation(NEIGHBOUR)) || holds(conn, way(OUTSIDE_STREET))
}

// a country that holds a city and a street, beside a country that holds nothing and a street that
// no area holds
pub(crate) fn with_isolated_areas(w: &world, name: &str) -> PathBuf {
  resolved_at(
    w,
    name,
    &[
      area(COUNTRY, 2, "Country", [-10.0, -10.0], [20.0, 10.0]),
      area(NEIGHBOUR, 2, "Neighbour", [30.0, -10.0], [40.0, 10.0]),
      area(CITY, 8, "City", [-2.0, -2.0], [5.0, 5.0]),
      street(INSIDE_STREET, "Inside Street", &[[0.8, 0.8], [1.6, 0.8]]),
      street(
        OUTSIDE_STREET,
        "Outside Street",
        &[[50.0, 0.8], [50.8, 0.8]],
      ),
    ],
  )
}

fn numbered(dir: &Path, node_id: i64, street_id: i64) {
  const SQL_INSERT: &str = "
    INSERT INTO house_numbers (
      node_id,
      admin_level_id,
      number,
      strategy
    ) VALUES (
      ?1,
      ?2,
      '10',
      0
    )
  ";

  writable(dir)
    .execute(SQL_INSERT, rusqlite::params![node_id, street_id])
    .expect("failed to insert a synthetic house number");
}

// a street of the fixture as the resolver leaves one that no area holds: a root
fn parentless(w: &world, name: &str) -> scratch {
  let s = resolved(w, name);
  let cut = writable(&s.dir)
    .execute(
      "UPDATE admin_levels_hierarchy SET parent_id = NULL WHERE admin_level_id = ?1",
      [way(UPPER_WAY)],
    )
    .expect("failed to cut the street from its areas");
  assert!(cut > 0, "{REGENERATE}");
  s
}

// 00.00. a country that holds nothing goes, and the one that holds a city stays
#[test]
#[ignore]
fn _00_00_a_country_without_children_is_deleted_and_one_with_children_stays() {
  let w = world();
  let dir = with_isolated_areas(w, "isolated_country");

  let out = isolated_deleted_at(w, &dir);

  assert!(
    plain(&out.stdout).contains(ONE_OF_EACH_DELETED),
    "{}",
    out.stdout
  );
  let conn = ledger(&dir);
  assert!(!holds(&conn, relation(NEIGHBOUR)));
  for kept in [COUNTRY, CITY] {
    assert!(holds(&conn, relation(kept)), "area {kept} must stay");
  }
}

// 00.01. a child is a child whatever it holds: a country whose states are empty stays
#[test]
#[ignore]
fn _00_01_a_country_whose_only_children_are_childless_states_stays() {
  let w = world();
  let dir = resolved_at(
    w,
    "isolated_country_of_states",
    &[
      area(COUNTRY, 2, "Country", [-10.0, -10.0], [20.0, 10.0]),
      area(STATE, 4, "State", [-2.0, -2.0], [5.0, 5.0]),
    ],
  );
  let before = snapshot(&ledger(&dir));

  let out = isolated_deleted_at(w, &dir);

  assert!(
    plain(&out.stdout).contains(NOTHING_ISOLATED),
    "{}",
    out.stdout
  );
  assert_eq!(snapshot(&ledger(&dir)), before);
}

// 00.02. a street that no area holds goes, and the one inside the city stays under it
#[test]
#[ignore]
fn _00_02_a_street_without_parents_is_deleted_and_one_inside_an_area_stays() {
  let w = world();
  let dir = with_isolated_areas(w, "isolated_street");
  assert!(parents_of(&ledger(&dir), way(OUTSIDE_STREET)).is_empty());

  isolated_deleted_at(w, &dir);

  let conn = ledger(&dir);
  assert!(!holds(&conn, way(OUTSIDE_STREET)));
  assert!(holds(&conn, way(INSIDE_STREET)));
  assert_eq!(parents_of(&conn, way(INSIDE_STREET)), vec![relation(CITY)]);
}

// 00.03. the numbers of a street that goes leave with it, and the run counts them
#[test]
#[ignore]
fn _00_03_the_house_numbers_of_a_deleted_street_go_with_it() {
  let w = world();
  let dir = with_isolated_areas(w, "isolated_street_numbers");
  numbered(&dir, 1, way(INSIDE_STREET));
  numbered(&dir, 2, way(OUTSIDE_STREET));
  numbered(&dir, 3, way(OUTSIDE_STREET));

  let out = isolated_deleted_at(w, &dir);

  assert!(
    plain(&out.stdout).contains("2 house numbers deleted"),
    "{}",
    out.stdout
  );
  let conn = ledger(&dir);
  assert_eq!(count(&conn, "SELECT COUNT(*) FROM house_numbers"), 1);
  assert_eq!(count(&conn, "SELECT node_id FROM house_numbers"), 1);
}

// 00.04. the rule names two levels: a state without children and a neighbourhood without parents
// are left alone
#[test]
#[ignore]
fn _00_04_the_other_levels_are_left_alone() {
  let w = world();
  let dir = resolved_at(
    w,
    "isolated_other_levels",
    &[
      area(COUNTRY, 2, "Country", [-10.0, -10.0], [20.0, 10.0]),
      area(STATE, 4, "State", [-2.0, -2.0], [5.0, 5.0]),
      area(
        NEIGHBOURHOOD,
        10,
        "Neighbourhood",
        [50.0, -2.0],
        [52.0, 0.0],
      ),
      street(
        OUTSIDE_STREET,
        "Outside Street",
        &[[60.0, 0.8], [60.8, 0.8]],
      ),
    ],
  );
  assert!(parents_of(&ledger(&dir), relation(NEIGHBOURHOOD)).is_empty());

  let out = isolated_deleted_at(w, &dir);

  assert!(
    plain(&out.stdout)
      .contains("deleted 0 countries without children and 1 streets without parents in"),
    "{}",
    out.stdout
  );
  let conn = ledger(&dir);
  assert!(!holds(&conn, way(OUTSIDE_STREET)));
  for kept in [COUNTRY, STATE, NEIGHBOURHOOD] {
    assert!(holds(&conn, relation(kept)), "area {kept} must stay");
  }
}

// 00.05. what goes takes its edges and its numbers, so the stage after it still finds a hierarchy
// that covers every row
#[test]
#[ignore]
fn _00_05_the_tables_stay_consistent_and_the_merge_stage_still_runs() {
  let w = world();
  let dir = with_isolated_areas(w, "isolated_consistent");
  numbered(&dir, 1, way(OUTSIDE_STREET));

  isolated_deleted_at(w, &dir);

  assert_the_tables_are_consistent(&ledger(&dir));
  let merged = stage(w, &dir, &["--preset", "brazil", "exec", MERGE_STAGE]);
  assert!(
    plain(&merged.stdout).contains("skipping merge-admin-levels"),
    "{}",
    merged.stdout
  );
}

// 00.06. a second run finds nothing isolated and leaves the indexes as they are
#[test]
#[ignore]
fn _00_06_a_second_run_changes_nothing_and_keeps_the_indexes() {
  let w = world();
  let s = parentless(w, "isolated_twice");
  isolated_deleted_at(w, &s.dir);

  assert_a_run_with_nothing_to_do_keeps_the_indexes(w, &s, isolated_deleted_at, NOTHING_ISOLATED);
}

// 00.07. what it deletes makes the two indexes after it stale: it clears them and says so
#[test]
#[ignore]
fn _00_07_the_indexes_after_it_are_cleared_and_have_to_be_recreated() {
  let w = world();
  let s = parentless(w, "isolated_clears");

  assert_the_indexes_are_cleared_and_recreated(w, &s, isolated_deleted_at, TEXT_QUERY);

  assert!(!holds(&s.ledger(), way(UPPER_WAY)));
  let result = query_at(w, &s.dir, "brazil", TEXT_QUERY);
  assert!(
    !matches(&result).is_empty(),
    "the ways of the street that kept their areas still answer"
  );
}

// 00.08. the ledger counts what the tables hold after the delete
#[test]
#[ignore]
fn _00_08_the_ledger_follows_the_table_after_the_delete() {
  let w = world();
  let s = parentless(w, "isolated_ledger");

  isolated_deleted_at(w, &s.dir);

  assert!(!holds(&s.ledger(), way(UPPER_WAY)));
  assert_the_ledger_follows_the_tables(&s.ledger());
}

// 00.09. the fixture is one country with every street inside it: the build deletes nothing
#[test]
#[ignore]
fn _00_09_the_fixture_holds_nothing_isolated() {
  let w = world();
  let s = resolved(w, "isolated_fixture");
  let before = snapshot(&s.ledger());

  let out = isolated_deleted_at(w, &s.dir);

  assert!(
    plain(&out.stdout).contains(NOTHING_ISOLATED),
    "{REGENERATE}:\n{}",
    out.stdout
  );
  assert_eq!(snapshot(&s.ledger()), before);
}

// 00.10. the indexes are cleared before the first delete and the run names the stages to run
// next; a run that deletes nothing clears nothing and names none
#[test]
#[ignore]
fn _00_10_only_a_run_that_deletes_clears_the_indexes_and_says_so() {
  let w = world();
  let dir = with_isolated_areas(w, "isolated_says_so");

  let deleting = plain(&isolated_deleted_at(w, &dir).stdout);
  assert_in_order(&deleting, &[CLEARING, ONE_OF_EACH_DELETED, NEXT_RUN]);

  let idle = plain(&isolated_deleted_at(w, &dir).stdout);
  assert!(idle.contains(NOTHING_ISOLATED), "{idle}");
  for unsaid in [CLEARING, NEXT_RUN] {
    assert!(
      !idle.contains(unsaid),
      "{unsaid:?} in a run that deleted nothing:\n{idle}"
    );
  }
}

// 00.11. a parent is a parent whatever its level: a street straight under a country stays, and
// the country, which holds nothing else, stays with it
#[test]
#[ignore]
fn _00_11_a_street_straight_under_a_country_stays_and_keeps_the_country() {
  let w = world();
  let dir = resolved_at(
    w,
    "isolated_street_under_country",
    &[
      area(COUNTRY, 2, "Country", [-10.0, -10.0], [20.0, 10.0]),
      street(INSIDE_STREET, "Inside Street", &[[0.8, 0.8], [1.6, 0.8]]),
    ],
  );
  assert_eq!(
    parents_of(&ledger(&dir), way(INSIDE_STREET)),
    vec![relation(COUNTRY)]
  );
  let before = snapshot(&ledger(&dir));

  let out = isolated_deleted_at(w, &dir);

  assert!(
    plain(&out.stdout).contains(NOTHING_ISOLATED),
    "{}",
    out.stdout
  );
  assert_eq!(snapshot(&ledger(&dir)), before);
}

// 00.12. the run counts every area it deletes, not the kinds of area
#[test]
#[ignore]
fn _00_12_every_isolated_area_is_counted() {
  let w = world();
  let dir = resolved_at(
    w,
    "isolated_counted",
    &[
      area(COUNTRY, 2, "Country", [-10.0, -10.0], [20.0, 10.0]),
      area(CITY, 8, "City", [-2.0, -2.0], [5.0, 5.0]),
      area(NEIGHBOUR, 2, "Neighbour", [30.0, -10.0], [40.0, 10.0]),
      area(FAR_COUNTRY, 2, "Far Country", [70.0, -10.0], [80.0, 10.0]),
      street(
        OUTSIDE_STREET,
        "Outside Street",
        &[[50.0, 0.8], [50.8, 0.8]],
      ),
      street(FAR_STREET, "Far Street", &[[60.0, 0.8], [60.8, 0.8]]),
    ],
  );

  let out = isolated_deleted_at(w, &dir);

  assert!(
    plain(&out.stdout)
      .contains("deleted 2 countries without children and 2 streets without parents in"),
    "{}",
    out.stdout
  );
  let conn = ledger(&dir);
  for kept in [COUNTRY, CITY] {
    assert!(holds(&conn, relation(kept)), "area {kept} must stay");
  }
  assert_eq!(count(&conn, "SELECT COUNT(*) FROM admin_levels"), 2);
}

// 01.00. it needs a resolved hierarchy: without one it says so and changes nothing
#[test]
#[ignore]
fn _01_00_it_refuses_a_database_without_a_hierarchy() {
  let w = world();
  let s = extracted(w, "isolated_no_hierarchy", "2", &[]);
  admin_levels_at(w, &s.dir, "2,4,8", &[]);
  let before = snapshot(&s.ledger());

  let out = w.geolite_in(&s.dir, &["--preset", "brazil", "exec", STAGE]);

  assert_refused(
    &out,
    "optimize-delete-isolated-admin-levels requires index-admin-levels-hierarchy",
  );
  assert_eq!(
    snapshot(&s.ledger()),
    before,
    "a refused run writes nothing"
  );
}

// 01.01. the rule has no floor: where every row is isolated the base ends empty, and the stage
// after it asks for the hierarchy that went with the rows
#[test]
#[ignore]
fn _01_01_a_base_where_every_row_is_isolated_ends_empty() {
  let w = world();
  let dir = resolved_at(
    w,
    "isolated_everything",
    &[
      area(COUNTRY, 2, "Country", [-10.0, -10.0], [20.0, 10.0]),
      street(
        OUTSIDE_STREET,
        "Outside Street",
        &[[50.0, 0.8], [50.8, 0.8]],
      ),
    ],
  );

  let out = isolated_deleted_at(w, &dir);

  assert!(
    plain(&out.stdout).contains(ONE_OF_EACH_DELETED),
    "{}",
    out.stdout
  );
  assert_eq!(count(&ledger(&dir), "SELECT COUNT(*) FROM admin_levels"), 0);
  let next = w.geolite_in(&dir, &["--preset", "brazil", "exec", MERGE_STAGE]);
  assert_refused(
    &next,
    "optimize-merge-admin-levels requires index-admin-levels-hierarchy",
  );
}
