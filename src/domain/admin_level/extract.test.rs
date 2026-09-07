use rusqlite::Connection;

use super::super::entity::admin_level;
use super::super::scale::level;
use super::{extract_event, extract_opts, extract_step, source, stage};
use crate::extract::pbf_fixtures::{
  NAME_PRIORITY, insert_closed_way, insert_relation, insert_way_at, memory_db,
};

const OPTS: extract_opts<'static> = extract_opts {
  threads: 1,
  name_priority: NAME_PRIORITY,
  rules: &[],
};

fn describe(event: &extract_event) -> String {
  let step = match &event.step {
    extract_step::started => "started".to_string(),
    extract_step::candidates { found, pending } => format!("candidates {found} found {pending} pending"),
    extract_step::progress(report) => format!("progress {}/{:?}", report.processed, report.total),
    extract_step::finished => "finished".to_string(),
  };
  format!(
    "{}/{} {:?} {step}",
    event.stage.ordinal, event.stage.of, event.stage.source
  )
}

fn events_of(conn: &Connection, level: level) -> Vec<String> {
  let mut seen = Vec::new();
  admin_level::extract(conn, level, &OPTS, |event| seen.push(describe(&event)));
  seen
}

fn insert_city(conn: &Connection) {
  insert_closed_way(conn, 300, 1, (0.0, 0.0), 1.0, &[]);
  insert_relation(
    conn,
    700,
    &[(1, 300, "outer")],
    &[
      ("name", "Cidade"),
      ("admin_level", "8"),
      ("boundary", "administrative"),
    ],
  );
}

#[test]
fn _00_a_level_below_neighborhood_has_one_relations_stage() {
  assert_eq!(
    admin_level::stages_of(level::city),
    vec![stage {
      level: level::city,
      source: source::relations,
      ordinal: 1,
      of: 1,
    }]
  );
}

#[test]
fn _01_neighborhood_runs_relations_then_place_ways() {
  let stages = admin_level::stages_of(level::neighborhood);
  assert_eq!(
    stages.iter().map(|s| s.source).collect::<Vec<_>>(),
    vec![source::relations, source::place_ways]
  );
  assert_eq!(stages[1].ordinal, 2);
  assert!(stages.iter().all(|s| s.of == 2 && s.level == level::neighborhood));
}

#[test]
fn _02_street_has_one_streets_stage() {
  let stages = admin_level::stages_of(level::street);
  assert_eq!(stages.len(), 1);
  assert_eq!(stages[0].source, source::streets);
}

#[test]
fn _03_a_relations_stage_reports_candidates_before_progress() {
  let conn = memory_db();
  insert_city(&conn);

  assert_eq!(
    events_of(&conn, level::city),
    vec![
      "1/1 relations started",
      "1/1 relations candidates 1 found 1 pending",
      "1/1 relations progress 0/Some(1)",
      "1/1 relations progress 1/Some(1)",
      "1/1 relations finished",
    ]
  );
  assert_eq!(
    events_of(&conn, level::city)[1],
    "1/1 relations candidates 1 found 0 pending",
    "a second run finds the same relation and nothing pending"
  );
}

#[test]
fn _04_neighborhood_emits_the_two_stages_in_order() {
  let conn = memory_db();

  assert_eq!(
    events_of(&conn, level::neighborhood),
    vec![
      "1/2 relations started",
      "1/2 relations candidates 0 found 0 pending",
      "1/2 relations progress 0/Some(0)",
      "1/2 relations finished",
      "2/2 place_ways started",
      "2/2 place_ways progress 0/Some(0)",
      "2/2 place_ways finished",
    ]
  );
}

#[test]
fn _05_a_streets_stage_emits_no_candidates_event() {
  let conn = memory_db();
  insert_way_at(
    &conn,
    10,
    1,
    &[(0.0, 0.0), (1.0, 0.0)],
    &[("name", "Rua Augusta"), ("highway", "residential")],
  );

  assert_eq!(
    events_of(&conn, level::street),
    vec![
      "1/1 streets started",
      "1/1 streets progress 0/Some(1)",
      "1/1 streets progress 1/Some(1)",
      "1/1 streets finished",
    ]
  );
}
