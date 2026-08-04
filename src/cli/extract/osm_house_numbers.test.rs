use super::command_handler_extract_osm_house_numbers;
use crate::extract::pbf_fixtures::{insert_node, insert_way_at, temp_scene};
use crate::presets::DEFAULT;

const SQL_COUNT_HOUSE_NUMBERS: &str = "
  SELECT COUNT(*)
  FROM house_numbers
";

fn house_number_count(db_path: &str) -> i64 {
  let conn = crate::database::open_write(db_path);
  conn
    .query_row(SQL_COUNT_HOUSE_NUMBERS, [], |r| r.get(0))
    .expect("failed to count house_numbers")
}

// a level-12 street plus one addr:housenumber node sitting right next to it.
fn street_with_candidate(db_path: &str) {
  let conn = crate::database::open_write(db_path);
  insert_way_at(
    &conn,
    100,
    1,
    &[(0.0, 0.0), (0.001, 0.0)],
    &[("highway", "residential"), ("name", "Rua Alfa")],
  );
  insert_node(
    &conn,
    50,
    0.0005,
    0.00002,
    &[("addr:housenumber", "100"), ("addr:street", "Rua Alfa")],
  );
  crate::extract::admin_levels::level_12::run(&conn, &[], &["name"], |_| {});
}

#[test]
fn _00_00_extracts_house_number_near_the_street() {
  let scene = temp_scene("cli_house_numbers_extract");
  street_with_candidate(&scene.db_path);
  command_handler_extract_osm_house_numbers(&scene.db_path, false, DEFAULT.extract_house_numbers);
  assert!(
    house_number_count(&scene.db_path) >= 1,
    "the candidate node must be attached to the street"
  );
}

#[test]
fn _00_01_recreate_clears_house_numbers_before_extracting() {
  let scene = temp_scene("cli_house_numbers_recreate");
  street_with_candidate(&scene.db_path);
  command_handler_extract_osm_house_numbers(&scene.db_path, false, DEFAULT.extract_house_numbers);
  let first = house_number_count(&scene.db_path);

  command_handler_extract_osm_house_numbers(&scene.db_path, true, DEFAULT.extract_house_numbers);
  assert_eq!(
    house_number_count(&scene.db_path),
    first,
    "recreate must rebuild house_numbers, not append to it"
  );
}

#[test]
fn _00_02_runs_on_an_empty_database_without_progress() {
  let scene = temp_scene("cli_house_numbers_empty");
  command_handler_extract_osm_house_numbers(&scene.db_path, false, DEFAULT.extract_house_numbers);
  assert_eq!(house_number_count(&scene.db_path), 0);
}
