use super::*;
use crate::extract::pbf_fixtures::{indexed_scene, temp_scene, tiny_pbf, write_pbf};

const SQL_COUNT_OSM_WAYS: &str = "
  SELECT COUNT(*)
  FROM osm_data.osm_ways
";

fn osm_way_count(db_path: &str) -> i64 {
  let conn = crate::database::open_write(db_path);
  conn
    .query_row(SQL_COUNT_OSM_WAYS, [], |r| r.get(0))
    .expect("failed to count osm_ways")
}

#[test]
fn _00_00_fmt_count_formats_units() {
  assert_eq!(fmt_count(5), "5");
  assert_eq!(fmt_count(999), "999");
  assert_eq!(fmt_count(1_500), "1.5K");
  assert_eq!(fmt_count(2_000_000), "2.0M");
}

#[test]
fn _00_01_fmt_bytes_formats_all_units() {
  assert_eq!(fmt_bytes(10), "10B");
  assert_eq!(fmt_bytes(2 * 1024), "2KiB");
  assert_eq!(fmt_bytes(3 * 1024 * 1024), "3.0MiB");
  assert_eq!(fmt_bytes(2 * 1024 * 1024 * 1024), "2.00GiB");
}

#[test]
fn _00_02_resolve_buffer_bytes_uses_the_explicit_limit() {
  assert_eq!(resolve_buffer_bytes(Some(2)), 2 * 1024 * 1024);
}

#[test]
fn _00_03_resolve_buffer_bytes_defaults_to_a_share_of_total_ram() {
  assert!(resolve_buffer_bytes(None) > 0);
}

#[test]
fn _01_00_extracts_rows_from_indexed_fixture_with_tag_lists() {
  let (scene, _file_id) = indexed_scene("cli_data_extract", &tiny_pbf());
  command_handler_extract_osm_pbf_data(
    &scene.guard.path.to_string_lossy(),
    &scene.db_path,
    &2,
    std::slice::from_ref(&scene.pbf_path),
    true,
    true,
    true,
    true,
    Some("name,highway,admin_level".to_string()),
    Some("bogus".to_string()),
    false,
    Some(8),
  );
  assert!(osm_way_count(&scene.db_path) > 0, "the fixture way must be decoded and written");
}

#[test]
fn _01_01_recreate_then_errors_when_no_blob_chunks_are_indexed() {
  let scene = temp_scene("cli_data_no_chunks");
  write_pbf(&scene.pbf_path, &tiny_pbf());
  // recreate on a fresh scene exercises the destroy branch; without a prior blob-chunks run the
  // handler reports the missing index and moves on instead of decoding.
  command_handler_extract_osm_pbf_data(
    &scene.guard.path.to_string_lossy(),
    &scene.db_path,
    &1,
    std::slice::from_ref(&scene.pbf_path),
    true,
    true,
    true,
    true,
    None,
    None,
    true,
    Some(4),
  );
  assert_eq!(osm_way_count(&scene.db_path), 0);
}

#[test]
fn _01_02_skips_unresolvable_input_and_continues_to_the_next() {
  let (scene, _file_id) = indexed_scene("cli_data_skip", &tiny_pbf());
  command_handler_extract_osm_pbf_data(
    &scene.guard.path.to_string_lossy(),
    &scene.db_path,
    &1,
    &["ghost-input".to_string(), scene.pbf_path.clone()],
    true,
    true,
    true,
    true,
    None,
    None,
    false,
    Some(4),
  );
  assert!(osm_way_count(&scene.db_path) > 0);
}

#[test]
fn _01_03_resolves_input_from_database_id() {
  let (scene, file_id) = indexed_scene("cli_data_resolve", &tiny_pbf());
  // the raw row id is not a filesystem path, forcing the "resolving ... done" branch.
  command_handler_extract_osm_pbf_data(
    &scene.guard.path.to_string_lossy(),
    &scene.db_path,
    &1,
    &[file_id.to_string()],
    true,
    true,
    true,
    true,
    None,
    None,
    false,
    Some(4),
  );
  assert!(osm_way_count(&scene.db_path) > 0);
}
