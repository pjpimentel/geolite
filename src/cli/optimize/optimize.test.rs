use super::*;
use crate::database::admin_levels::{admin_levels as admin_levels_row, batch_upsert};
use crate::database::{open_write, osm_data_path};
use crate::extract::pbf_fixtures::tempdir_guard;
use geo::{Coord, Geometry, LineString};

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

fn scene(tag: &str, with_streets: bool, with_hierarchy: bool) -> (tempdir_guard, String, String) {
  let guard = tempdir_guard::new(tag);
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  let index = guard.path.join("idx.tantivy").to_string_lossy().into_owned();
  let conn = open_write(&db);
  if with_streets {
    batch_upsert(
      &conn,
      &[street_row("Rua Alpha", 1, 0.0), street_row("Rua Beta", 2, 0.0005)],
    );
  }
  if with_hierarchy {
    crate::index::hierarchy::run(&conn, |_| {});
  }
  (guard, db, index)
}

#[test]
fn _00_00_fmt_size_formats_every_unit() {
  assert_eq!(fmt_size(500), "500 b");
  assert_eq!(fmt_size(2 * 1024), "2.00 kb");
  assert_eq!(fmt_size(3 * 1024 * 1024), "3.00 mb");
  assert_eq!(fmt_size(2 * 1024 * 1024 * 1024), "2.00 gb");
}

#[test]
fn _00_01_file_size_is_zero_when_missing_and_len_when_present() {
  let guard = tempdir_guard::new("cli_optimize_file_size");
  let path = guard.path.join("f.bin");
  assert_eq!(file_size(&path.to_string_lossy()), 0);
  std::fs::write(&path, b"12345").expect("failed to write probe file");
  assert_eq!(file_size(&path.to_string_lossy()), 5);
}

#[test]
fn _00_02_dir_size_sums_files_recursively_and_is_zero_when_missing() {
  let guard = tempdir_guard::new("cli_optimize_dir_size");
  assert_eq!(dir_size(&guard.path.join("missing").to_string_lossy()), 0);
  std::fs::write(guard.path.join("a.bin"), b"123").expect("failed to write file");
  let sub = guard.path.join("sub");
  std::fs::create_dir_all(&sub).expect("failed to create subdir");
  std::fs::write(sub.join("b.bin"), b"12345").expect("failed to write nested file");
  // a dangling symlink has no metadata and must be skipped, not counted.
  #[cfg(unix)]
  std::os::unix::fs::symlink(guard.path.join("missing"), guard.path.join("dangling"))
    .expect("failed to create dangling symlink");
  assert_eq!(dir_size(&guard.path.to_string_lossy()), 8);
}

#[test]
fn _01_00_default_run_returns_early_when_admin_levels_is_empty() {
  let (guard, db, index) = scene("cli_optimize_empty_levels", false, false);
  command_handler_optimize(&guard.path.to_string_lossy(), &db, &index, None);
  assert!(
    std::path::Path::new(&osm_data_path(&db)).exists(),
    "nothing must be deleted when admin_levels is empty"
  );
}

#[test]
fn _01_01_default_run_returns_early_when_hierarchy_is_empty() {
  let (guard, db, index) = scene("cli_optimize_empty_hierarchy", true, false);
  command_handler_optimize(&guard.path.to_string_lossy(), &db, &index, None);
  assert!(
    std::path::Path::new(&osm_data_path(&db)).exists(),
    "nothing must be deleted when the hierarchy is empty"
  );
}

#[test]
fn _01_02_dispatches_the_delete_intermediary_data_leaf() {
  let (guard, db, index) = scene("cli_optimize_delete_leaf", true, true);
  command_handler_optimize(
    &guard.path.to_string_lossy(),
    &db,
    &index,
    Some(optimize_commands::delete_intermediary_data),
  );
  assert!(
    !std::path::Path::new(&osm_data_path(&db)).exists(),
    "the leaf must delete the osm_data sibling"
  );
}

#[test]
fn _01_03_dispatches_the_sqlite_file_leaf_ignoring_the_positional() {
  let (guard, db, index) = scene("cli_optimize_sqlite_leaf", true, false);
  command_handler_optimize(
    &guard.path.to_string_lossy(),
    &db,
    &index,
    Some(optimize_commands::sqlite_file { pbf_or_sqlite: "ignored.pbf".to_string() }),
  );
  assert!(std::path::Path::new(&db).exists(), "the database must survive the vacuum");
}

#[test]
fn _01_04_delete_intermediary_leaf_returns_early_on_missing_prerequisites() {
  let (guard, db, _index) = scene("cli_optimize_leaf_no_levels", false, false);
  delete_intermediary_data::command_handler_optimize_delete_intermediary_data(
    &guard.path.to_string_lossy(),
    &db,
  );
  assert!(std::path::Path::new(&osm_data_path(&db)).exists());

  let (guard, db, _index) = scene("cli_optimize_leaf_no_hierarchy", true, false);
  delete_intermediary_data::command_handler_optimize_delete_intermediary_data(
    &guard.path.to_string_lossy(),
    &db,
  );
  assert!(std::path::Path::new(&osm_data_path(&db)).exists());
}
