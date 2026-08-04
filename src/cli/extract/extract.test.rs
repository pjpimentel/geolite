use super::*;
use crate::extract::pbf_fixtures::{
  blob_compression, block_spec, data_chunk, header_chunk, node, stored_admin_levels, temp_scene,
  way, write_pbf,
};
use crate::presets::DEFAULT;

// header + two street nodes + the street way: the smallest input that survives the whole
// blob-chunks -> osm-data -> admin-levels chain and produces a level-12 row.
fn street_chunks() -> Vec<Vec<u8>> {
  vec![
    header_chunk(),
    data_chunk(
      &block_spec {
        dense: vec![node(1, -23.9724, -46.3198, &[]), node(2, -23.9724, -46.3197, &[])],
        ..Default::default()
      },
      blob_compression::zlib,
    ),
    data_chunk(
      &block_spec {
        ways: vec![way(100, &[1, 2], &[("highway", "residential"), ("name", "Rua Teste")])],
        ..Default::default()
      },
      blob_compression::zlib,
    ),
  ]
}

#[test]
fn _00_00_parse_name_priority_accepts_single_tag() {
  assert_eq!(parse_name_priority("name"), Ok(vec!["name"]));
}

#[test]
fn _00_01_parse_name_priority_accepts_list_with_spaces_colon_dash_underscore() {
  assert_eq!(
    parse_name_priority(" name:pt-BR , int_name , name "),
    Ok(vec!["name:pt-BR", "int_name", "name"])
  );
}

#[test]
fn _00_02_parse_name_priority_rejects_empty_input() {
  let err = parse_name_priority(" , ").expect_err("empty list must be rejected");
  assert!(err.contains("at least one tag required"), "unexpected error: {err}");
}

#[test]
fn _00_03_parse_name_priority_rejects_whitespace_inside_tag() {
  let err = parse_name_priority("bad tag").expect_err("inner whitespace must be rejected");
  assert!(err.contains("invalid characters in tag 'bad tag'"), "unexpected error: {err}");
}

#[test]
fn _00_04_parse_name_priority_rejects_quote_characters() {
  let err = parse_name_priority("name\"").expect_err("quotes must be rejected");
  assert!(err.contains("invalid characters"), "unexpected error: {err}");
}

#[test]
fn _01_00_dispatches_blob_chunks_and_header() {
  let scene = temp_scene("cli_dispatch_chunks_header");
  write_pbf(&scene.pbf_path, &street_chunks());
  let data_path = scene.guard.path.to_string_lossy().into_owned();

  command_handler_extract(
    &data_path,
    &1,
    &scene.db_path,
    extract_commands::osm_pbf_blob_chunks {
      inputs: vec![scene.pbf_path.clone()],
      recreate: true,
    },
    &false,
    &DEFAULT,
  );
  command_handler_extract(
    &data_path,
    &1,
    &scene.db_path,
    extract_commands::osm_pbf_header { inputs: vec![scene.pbf_path.clone()] },
    &false,
    &DEFAULT,
  );

  let conn = crate::database::open_write(&scene.db_path);
  let file_id = crate::database::osm_pbf_files::ensure_by_file_path(&conn, &scene.pbf_path);
  assert!(
    crate::database::osm_pbf_blob_chunks::count_by_file_id(&conn, file_id) > 0,
    "blob chunks must be extracted through the dispatcher"
  );
}

#[test]
fn _01_01_dispatches_osm_pbf_data_and_admin_levels_with_explicit_options() {
  let scene = temp_scene("cli_dispatch_data_levels");
  write_pbf(&scene.pbf_path, &street_chunks());
  let data_path = scene.guard.path.to_string_lossy().into_owned();

  command_handler_extract(
    &data_path,
    &1,
    &scene.db_path,
    extract_commands::osm_pbf_blob_chunks {
      inputs: vec![scene.pbf_path.clone()],
      recreate: false,
    },
    &false,
    &DEFAULT,
  );
  command_handler_extract(
    &data_path,
    &2,
    &scene.db_path,
    extract_commands::osm_pbf_data {
      inputs: vec![scene.pbf_path.clone()],
      include_relations: true,
      include_ways: true,
      include_nodes: true,
      ignore_info: true,
      tags_include_list: Some("name,highway".to_string()),
      tags_ignore_list: Some("bogus".to_string()),
      recreate: false,
      buffer_limit_in_mb: Some(8),
    },
    &false,
    &DEFAULT,
  );
  // "x" is dropped by the level parser and the duplicated 12 keeps the level list explicit.
  command_handler_extract(
    &data_path,
    &1,
    &scene.db_path,
    extract_commands::osm_admin_levels {
      admin_level: Some("12, x, 12".to_string()),
      recreate: false,
      name_priority: Some(" name ".to_string()),
    },
    &false,
    &DEFAULT,
  );

  let conn = crate::database::open_write(&scene.db_path);
  let stored = stored_admin_levels(&conn);
  assert!(
    stored.iter().any(|(way_id, level, name)| {
      *way_id == Some(100) && *level == 12 && name == "Rua Teste"
    }),
    "the street must be extracted end to end, got {stored:?}"
  );
}

#[test]
fn _01_02_dispatches_admin_levels_with_preset_defaults() {
  let scene = temp_scene("cli_dispatch_levels_preset");
  command_handler_extract(
    &scene.guard.path.to_string_lossy(),
    &1,
    &scene.db_path,
    extract_commands::osm_admin_levels {
      admin_level: None,
      recreate: false,
      name_priority: None,
    },
    &false,
    &DEFAULT,
  );

  let conn = crate::database::open_write(&scene.db_path);
  assert!(
    stored_admin_levels(&conn).is_empty(),
    "an empty database must produce no admin_levels rows"
  );
}

#[test]
fn _01_03_dispatches_house_numbers() {
  let scene = temp_scene("cli_dispatch_house_numbers");
  command_handler_extract(
    &scene.guard.path.to_string_lossy(),
    &1,
    &scene.db_path,
    extract_commands::osm_house_numbers { recreate: true },
    &false,
    &DEFAULT,
  );
}

#[test]
#[ignore] // executed only as a child of _02_00
fn _90_admin_levels_invalid_name_priority() {
  let scene = temp_scene("cli_bad_name_priority");
  command_handler_extract(
    &scene.guard.path.to_string_lossy(),
    &1,
    &scene.db_path,
    extract_commands::osm_admin_levels {
      admin_level: Some("12".to_string()),
      recreate: false,
      name_priority: Some("bad tag!".to_string()),
    },
    &false,
    &DEFAULT,
  );
}

#[test]
fn _02_00_invalid_name_priority_exits_one() {
  let out = crate::cli::tests::respawn(
    "cli::extract::tests::_90_admin_levels_invalid_name_priority",
    &[],
    &[],
  );
  assert_eq!(out.status.code(), Some(1), "stderr: {}", crate::cli::tests::stderr_of(&out));
  assert!(crate::cli::tests::stderr_of(&out).contains("invalid --name-priority"));
}
