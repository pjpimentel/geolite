use super::*;
use crate::extract::admin_levels::progress_report;
use crate::extract::pbf_fixtures::{
  insert_closed_way, insert_relation, insert_way_at, stored_admin_levels, temp_scene,
};

#[test]
fn _00_00_level_name_maps_every_known_level_and_unknown() {
  for (level, name) in [
    (1u8, "continent"),
    (2, "country"),
    (3, "region"),
    (4, "state"),
    (5, "district"),
    (6, "county"),
    (7, "municipality"),
    (8, "city"),
    (9, "locality"),
    (10, "neighborhood"),
    (12, "street"),
    (14, "address"),
    (11, "unknown"),
  ] {
    assert_eq!(level_name(level), name, "level {level}");
  }
}

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

#[test]
fn _02_00_generic_level_extracts_a_relation() {
  let scene = temp_scene("cli_levels_generic");
  {
    let conn = crate::database::open_write(&scene.db_path);
    insert_closed_way(&conn, 300, 1, (0.0, 0.0), 1.0, &[]);
    insert_relation(
      &conn,
      700,
      &[(1, 300, "outer")],
      &[("name", "Cidade"), ("admin_level", "8"), ("boundary", "administrative")],
    );
  }
  command_handler_extract_osm_admin_levels(&scene.db_path, &[8], &1, false, &["name"], &[]);

  let conn = crate::database::open_write(&scene.db_path);
  let stored = stored_admin_levels(&conn);
  assert!(
    stored.iter().any(|(_, level, name)| *level == 8 && name == "Cidade"),
    "the level-8 relation must be stored, got {stored:?}"
  );
}

#[test]
fn _02_01_level_10_runs_both_relation_and_way_stages() {
  let scene = temp_scene("cli_levels_ten");
  {
    let conn = crate::database::open_write(&scene.db_path);
    insert_closed_way(&conn, 310, 11, (0.0, 0.0), 0.5, &[]);
    insert_relation(
      &conn,
      710,
      &[(1, 310, "outer")],
      &[("name", "Bairro"), ("admin_level", "10"), ("boundary", "administrative")],
    );
  }
  command_handler_extract_osm_admin_levels(&scene.db_path, &[10], &1, false, &["name"], &[]);

  let conn = crate::database::open_write(&scene.db_path);
  let stored = stored_admin_levels(&conn);
  assert!(
    stored.iter().any(|(_, level, name)| *level == 10 && name == "Bairro"),
    "the level-10 relation must be stored, got {stored:?}"
  );
}

#[test]
fn _02_02_unsupported_level_is_skipped_and_the_next_level_runs() {
  let scene = temp_scene("cli_levels_unsupported");
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
  // 11 has no osm_admin_level mapping and must not stop 12 from extracting.
  command_handler_extract_osm_admin_levels(&scene.db_path, &[11, 12], &1, false, &["name"], &[]);

  let conn = crate::database::open_write(&scene.db_path);
  let stored = stored_admin_levels(&conn);
  assert!(
    stored.iter().any(|(way_id, level, name)| {
      *way_id == Some(100) && *level == 12 && name == "Rua Alfa"
    }),
    "the street must be stored even after the unsupported level, got {stored:?}"
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
  command_handler_extract_osm_admin_levels(&scene.db_path, &[12], &1, false, &["name"], &[]);
  command_handler_extract_osm_admin_levels(&scene.db_path, &[12], &1, true, &["name"], &[]);

  let conn = crate::database::open_write(&scene.db_path);
  assert_eq!(
    stored_admin_levels(&conn).len(),
    1,
    "recreate must rebuild admin_levels, not append to it"
  );
}
