use crate::domain::admin_level::level;
use super::*;
use crate::extract::admin_levels::progress_report;
use crate::extract::pbf_fixtures::{
  insert_closed_way, insert_relation, insert_way_at, stored_admin_levels, temp_scene,
};

#[test]
fn _01_00_extract_stage_skips_when_there_is_nothing_to_extract() {
  let elapsed = extract_stage("nothing", |on_progress| {
    on_progress(progress_report { total: Some(0), processed: 0 });
  });
  assert_eq!(elapsed, 0.0);
}

#[test]
fn _01_01_extract_stage_reports_the_extracted_total() {
  let elapsed = extract_stage("things", |on_progress| {
    on_progress(progress_report { total: Some(3), processed: 1 });
    on_progress(progress_report { total: None, processed: 2 });
    on_progress(progress_report { total: Some(3), processed: 3 });
  });
  assert!(elapsed > 0.0, "a stage with work must report elapsed time");
}

// inserts a closed-way relation tagged at `level`, runs the handler for that level only and
// returns the stored rows — the shared scene for the per-level scenarios below.
fn extract_relation_at_level(
  tag: &str,
  level: level,
  name: &str,
) -> Vec<(Option<u64>, u8, String)> {
  let scene = temp_scene(tag);
  let level_value = level.value().to_string();
  {
    let conn = crate::database::open_write(&scene.db_path);
    insert_closed_way(&conn, 300, 1, (0.0, 0.0), 1.0, &[]);
    insert_relation(
      &conn,
      700,
      &[(1, 300, "outer")],
      &[("name", name), ("admin_level", level_value.as_str()), ("boundary", "administrative")],
    );
  }
  command_handler_extract_osm_admin_levels(&scene.db_path, &[level], &1, false, &["name"], &[]);

  let conn = crate::database::open_write(&scene.db_path);
  stored_admin_levels(&conn)
}

#[test]
fn _02_00_generic_level_extracts_a_relation() {
  let stored = extract_relation_at_level("cli_levels_generic", level::city, "Cidade");
  assert!(
    stored.iter().any(|(_, level, name)| *level == 8 && name == "Cidade"),
    "the level-8 relation must be stored, got {stored:?}"
  );
}

#[test]
fn _02_01_level_10_runs_both_relation_and_way_stages() {
  let stored = extract_relation_at_level("cli_levels_ten", level::neighborhood, "Bairro");
  assert!(
    stored.iter().any(|(_, level, name)| *level == 10 && name == "Bairro"),
    "the level-10 relation must be stored, got {stored:?}"
  );
}

#[test]
fn _02_03_recreate_destroys_previous_admin_levels() {
  let scene = temp_scene("cli_levels_recreate");
  {
    let conn = crate::database::open_write(&scene.db_path);
    insert_way_at(
      &conn,
      100,
      1,
      &[(0.0, 0.0), (0.001, 0.0)],
      &[("highway", "residential"), ("name", "Rua Alfa")],
    );
  }
  command_handler_extract_osm_admin_levels(&scene.db_path, &[level::street], &1, false, &["name"], &[]);
  command_handler_extract_osm_admin_levels(&scene.db_path, &[level::street], &1, true, &["name"], &[]);

  let conn = crate::database::open_write(&scene.db_path);
  assert_eq!(
    stored_admin_levels(&conn).len(),
    1,
    "recreate must rebuild admin_levels, not append to it"
  );
}
