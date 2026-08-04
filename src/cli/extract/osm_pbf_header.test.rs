use super::command_handler_extract_osm_pbf_header;
use crate::extract::pbf_fixtures::{
  blob_compression, header_blob, header_chunk, header_spec, indexed_scene, make_chunk,
};

#[test]
fn _00_00_prints_every_header_field_from_the_default_spec() {
  let (scene, _file_id) = indexed_scene("cli_header_full", &[header_chunk()]);
  command_handler_extract_osm_pbf_header(
    &scene.guard.path.to_string_lossy(),
    &scene.db_path,
    std::slice::from_ref(&scene.pbf_path),
  );
}

#[test]
fn _00_01_omits_optional_lines_when_header_fields_are_absent() {
  let bare = header_spec {
    bbox: None,
    writingprogram: None,
    source: None,
    replication_timestamp: None,
    ..Default::default()
  };
  let (scene, _file_id) = indexed_scene(
    "cli_header_bare",
    &[make_chunk("OSMHeader", &header_blob(&bare, blob_compression::zlib))],
  );
  command_handler_extract_osm_pbf_header(
    &scene.guard.path.to_string_lossy(),
    &scene.db_path,
    std::slice::from_ref(&scene.pbf_path),
  );
}

#[test]
fn _00_02_resolves_input_from_database_id() {
  let (scene, file_id) = indexed_scene("cli_header_resolve", &[header_chunk()]);
  // the raw row id is not a filesystem path, forcing the "resolving ... done" branch.
  command_handler_extract_osm_pbf_header(
    &scene.guard.path.to_string_lossy(),
    &scene.db_path,
    &[file_id.to_string()],
  );
}

#[test]
fn _00_03_skips_unresolvable_input_and_continues_to_the_next() {
  let (scene, _file_id) = indexed_scene("cli_header_skip", &[header_chunk()]);
  command_handler_extract_osm_pbf_header(
    &scene.guard.path.to_string_lossy(),
    &scene.db_path,
    &["ghost-input".to_string(), scene.pbf_path.clone()],
  );
}
