use super::command_handler_query;
use crate::database::admin_levels::{admin_levels as admin_levels_row, batch_upsert};
use crate::database::open_write;
use crate::extract::pbf_fixtures::tempdir_guard;
use crate::index::admin_levels_hierarchy_tantivy as tantivy;
use crate::presets::DEFAULT;
use geo::{Coord, Geometry, LineString};
use std::path::Path;

fn street_row(name: &str, way_id: u64, lon_offset: f64) -> admin_levels_row {
  admin_levels_row {
    relation_id: None,
    way_id: Some(way_id),
    admin_level: 12,
    wkb: Geometry::LineString(LineString(vec![
      Coord { x: -46.3198 + lon_offset, y: -23.9724 },
      Coord { x: -46.3197 + lon_offset, y: -23.9724 },
    ]))
    .into(),
    name: name.to_string(),
    country_iso_code: None,
    post_code: None,
  }
}

// two indexed streets on disk: sqlite + coordinates + hierarchy + tantivy, everything the
// query handler loads by path.
fn indexed_scene(tag: &str) -> (tempdir_guard, String, String) {
  let guard = tempdir_guard::new(tag);
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  let index = guard.path.join("idx.tantivy").to_string_lossy().into_owned();
  let conn = open_write(&db);
  batch_upsert(
    &conn,
    &[street_row("Rua Alpha", 1, 0.0), street_row("Rua Beta", 2, 0.0005)],
  );
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  tantivy::build(&conn, Path::new(&index), DEFAULT.index_user_friendly_name.boosts, &[]);
  (guard, db, index)
}

#[test]
fn _00_00_prints_json_for_a_text_query_with_defaults() {
  let (_guard, db, index) = indexed_scene("cli_query_defaults");
  command_handler_query(
    &db,
    &index,
    "Rua Alpha",
    None,
    None,
    None,
    None,
    true,
    DEFAULT.index_user_friendly_name.boosts,
  );
}

#[test]
fn _00_01_applies_every_optional_filter() {
  let (_guard, db, index) = indexed_scene("cli_query_filters");
  let bounding = crate::http::parse_bounding_wkt("POLYGON((-47 -24,-46 -24,-46 -23,-47 -23,-47 -24))")
    .expect("valid polygon");
  command_handler_query(
    &db,
    &index,
    "Rua Beta",
    Some("{admin_level_12_name}"),
    Some(0.1),
    Some(bounding),
    Some(vec![12]),
    false,
    DEFAULT.index_user_friendly_name.boosts,
  );
}

#[test]
#[ignore] // executed only as a child of _01_00
fn _90_query_without_tantivy_index() {
  let guard = tempdir_guard::new("cli_query_no_index");
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  drop(open_write(&db));
  command_handler_query(
    &db,
    &guard.path.join("missing.tantivy").to_string_lossy(),
    "anything",
    None,
    None,
    None,
    None,
    true,
    DEFAULT.index_user_friendly_name.boosts,
  );
}

#[test]
fn _01_00_missing_tantivy_index_exits_one() {
  let out = crate::cli::tests::respawn("cli::query::tests::_90_query_without_tantivy_index", &[], &[]);
  assert_eq!(out.status.code(), Some(1), "stderr: {}", crate::cli::tests::stderr_of(&out));
  assert!(crate::cli::tests::stderr_of(&out).contains("tantivy index not found"));
}
