use super::command_handler_extract_osm_pbf_blob_chunks;
use crate::extract::pbf_fixtures::{temp_scene, tiny_pbf, write_pbf};

fn chunk_count(db_path: &str, pbf_path: &str) -> i64 {
  let conn = crate::database::open_write(db_path);
  let file_id = crate::database::osm_pbf_files::ensure_by_file_path(&conn, pbf_path);
  crate::database::osm_pbf_blob_chunks::count_by_file_id(&conn, file_id)
}

#[test]
fn _00_00_extracts_chunks_from_the_fixture() {
  let scene = temp_scene("cli_chunks_extract");
  write_pbf(&scene.pbf_path, &tiny_pbf());
  command_handler_extract_osm_pbf_blob_chunks(
    &scene.guard.path.to_string_lossy(),
    &scene.db_path,
    std::slice::from_ref(&scene.pbf_path),
    false,
  );
  assert!(chunk_count(&scene.db_path, &scene.pbf_path) > 0);
}

#[test]
fn _00_01_recreate_destroys_previous_data_before_extracting() {
  let scene = temp_scene("cli_chunks_recreate");
  write_pbf(&scene.pbf_path, &tiny_pbf());
  let data_path = scene.guard.path.to_string_lossy().into_owned();

  command_handler_extract_osm_pbf_blob_chunks(&data_path, &scene.db_path, std::slice::from_ref(&scene.pbf_path), false);
  let first = chunk_count(&scene.db_path, &scene.pbf_path);

  command_handler_extract_osm_pbf_blob_chunks(&data_path, &scene.db_path, std::slice::from_ref(&scene.pbf_path), true);
  assert_eq!(
    chunk_count(&scene.db_path, &scene.pbf_path),
    first,
    "recreate must rebuild the chunk index, not append to it"
  );
}

#[test]
fn _00_02_skips_unresolvable_input_and_continues_to_the_next() {
  let scene = temp_scene("cli_chunks_skip");
  write_pbf(&scene.pbf_path, &tiny_pbf());
  command_handler_extract_osm_pbf_blob_chunks(
    &scene.guard.path.to_string_lossy(),
    &scene.db_path,
    &["ghost-input".to_string(), scene.pbf_path.clone()],
    false,
  );
  assert!(chunk_count(&scene.db_path, &scene.pbf_path) > 0);
}

#[test]
fn _00_03_resolves_input_from_database_id() {
  let scene = temp_scene("cli_chunks_resolve");
  write_pbf(&scene.pbf_path, &tiny_pbf());
  let file_id = {
    let conn = crate::database::open_write(&scene.db_path);
    crate::database::osm_pbf_files::ensure_by_file_path(&conn, &scene.pbf_path)
  };
  command_handler_extract_osm_pbf_blob_chunks(
    &scene.guard.path.to_string_lossy(),
    &scene.db_path,
    &[file_id.to_string()],
    false,
  );
  assert!(chunk_count(&scene.db_path, &scene.pbf_path) > 0);
}
