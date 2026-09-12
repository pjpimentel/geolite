use std::cell::RefCell;
use std::path::Path;

use rusqlite::Connection;

use super::super::entity::hierarchy_row;
use super::super::fixtures::{area, street};
use super::super::repository::batch_insert;
use super::testing::{build_test_index, tempdir_guard};
use super::{
  build, build_entity_text, default_path_for, destroy, expand_abbreviations, fuzzy_distance_for,
  load, run, tokenize,
};
use crate::domain::admin_level::id::admin_level_id;
use crate::domain::admin_level::repository::batch_upsert;
use crate::domain::admin_level::{admin_level, level};
use crate::presets::DEFAULT;

const RUA_AUGUSTA: i64 = 2;
const RUA_ANGUSTA: i64 = 4;
const AUGUSTA: i64 = 15;

// one hierarchy row per area, each its own root, so that the index has one document per area
fn indexed(rows: Vec<admin_level>) -> Connection {
  let conn = crate::database::open_write(":memory:");
  let hierarchy: Vec<hierarchy_row> = rows
    .iter()
    .map(|r| {
      let id = match (r.relation_id, r.way_id) {
        (Some(relation), _) => admin_level_id::from_relation(relation),
        (None, Some(way)) => admin_level_id::from_way(way),
        (None, None) => unreachable!(),
      };
      hierarchy_row {
        admin_level_id: id.raw() as i64,
        ancestor_ids: "[]".to_string(),
        user_friendly_name: r.name.clone(),
      }
    })
    .collect();
  batch_upsert(&conn, &rows);
  batch_insert(&conn, &hierarchy);
  conn
}

fn ids(hits: Vec<(i64, f32)>) -> Vec<i64> {
  let mut ids: Vec<i64> = hits.into_iter().map(|(id, _)| id).collect();
  ids.sort_unstable();
  ids
}

#[test]
fn _00_the_entity_text_carries_the_post_code_in_both_forms() {
  assert_eq!(build_entity_text("Rua A", None), "Rua A");
  assert_eq!(build_entity_text("Rua A", Some("  ")), "Rua A");
  assert_eq!(build_entity_text("Rua A", Some("01310-100")), "Rua A 01310-100 01310100");
  assert_eq!(build_entity_text("Rua A", Some("12345")), "Rua A 12345", "digits only once");
}

#[test]
fn _01_abbreviations_expand_in_both_directions() {
  let abbreviations = [("r.", "rua")];
  assert_eq!(expand_abbreviations("Rua Augusta", &abbreviations), "Rua Augusta r. augusta");
  assert_eq!(expand_abbreviations("R. Augusta", &abbreviations), "R. Augusta rua augusta");
  assert_eq!(expand_abbreviations("Rua Augusta", &[]), "Rua Augusta");
}

#[test]
fn _02_the_fuzzy_distance_depends_on_the_token_length() {
  assert_eq!(fuzzy_distance_for("100"), 1);
  assert_eq!(fuzzy_distance_for("abcd"), 2);
  assert_eq!(fuzzy_distance_for("praça"), 2);
}

#[test]
fn _03_tokenize_folds_case_and_diacritics() {
  assert_eq!(tokenize("Praça Sé, 01310-100"), vec!["praca", "se", "01310", "100"]);
  assert!(tokenize("  ").is_empty());
}

#[test]
fn _04_the_default_index_path_sits_beside_the_database() {
  assert_eq!(default_path_for(":memory:"), None);
  assert_eq!(default_path_for(""), None);
  assert_eq!(
    default_path_for("/data/database.sqlite3"),
    Some(Path::new("/data/database.tantivy").to_path_buf())
  );
}

#[test]
fn _05_load_answers_none_where_no_index_exists() {
  let guard = tempdir_guard::new();
  let boosts = DEFAULT.index_user_friendly_name.boosts;
  assert!(load(&guard.path, boosts).is_none());

  let conn = indexed(vec![street(1, "Rua Augusta", 0.0)]);
  build(&conn, &guard.path, boosts, &[]);
  assert!(load(&guard.path, boosts).is_some());

  destroy(&guard.path);
  assert!(load(&guard.path, boosts).is_none());
}

#[test]
fn _06_a_last_admin_levels_filter_keeps_only_those_levels() {
  let conn = indexed(vec![street(1, "Rua Augusta", 0.0), area(7, level::city, "Augusta", 0.0, 1.0)]);
  let (_guard, index) = build_test_index(&conn);

  assert_eq!(ids(index.search("augusta", 10, None, None)), vec![RUA_AUGUSTA, AUGUSTA]);
  assert_eq!(
    ids(index.search("augusta", 10, Some(&[level::street]), None)),
    vec![RUA_AUGUSTA]
  );
  assert_eq!(
    ids(index.search("augusta", 10, Some(&[level::city, level::country]), None)),
    vec![AUGUSTA]
  );
}

#[test]
fn _07_an_allowed_ids_filter_restricts_the_hits() {
  let conn = indexed(vec![street(1, "Rua Augusta", 0.0), area(7, level::city, "Augusta", 0.0, 1.0)]);
  let (_guard, index) = build_test_index(&conn);

  assert_eq!(ids(index.search("augusta", 10, None, Some(&[AUGUSTA]))), vec![AUGUSTA]);
  assert!(index.search("augusta", 10, None, Some(&[])).is_empty());
}

#[test]
fn _08_a_strict_match_wins_over_a_fuzzy_one() {
  let conn = indexed(vec![
    street(1, "Rua Augusta", 0.0),
    street(2, "Rua Angusta", 1.0),
  ]);
  let (_guard, index) = build_test_index(&conn);

  assert_eq!(ids(index.search("rua augusta", 10, None, None)), vec![RUA_AUGUSTA]);
  assert_eq!(
    ids(index.search("rua agusta", 10, None, None)),
    vec![RUA_AUGUSTA, RUA_ANGUSTA],
    "a typo falls back to the loose query, which fuzzy-matches both"
  );
  assert!(index.search("", 10, None, None).is_empty());
}

#[test]
fn _09_run_reports_the_hierarchy_count_around_the_build() {
  let guard = tempdir_guard::new();
  let conn = indexed(vec![street(1, "Rua Augusta", 0.0), area(7, level::city, "Augusta", 0.0, 1.0)]);
  let seen = RefCell::new(Vec::new());
  let index = run(&conn, &guard.path, &DEFAULT.index_user_friendly_name, |p| {
    seen.borrow_mut().push((p.total, p.processed))
  });

  assert_eq!(seen.into_inner(), vec![(Some(2), 0), (Some(2), 2)]);
  assert_eq!(ids(index.search("augusta", 10, None, None)).len(), 2);
}
