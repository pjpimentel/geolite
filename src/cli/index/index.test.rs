use super::*;
use crate::cli::tests::street_row;
use crate::database::admin_levels::batch_upsert;
use crate::database::open_write;
use crate::extract::pbf_fixtures::tempdir_guard;
use crate::presets::DEFAULT;

// a scene with two streets in admin_levels — enough for every index stage to have work.
fn street_scene(tag: &str) -> (tempdir_guard, String, String) {
  let guard = tempdir_guard::new(tag);
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  let index = guard.path.join("idx.tantivy").to_string_lossy().into_owned();
  let conn = open_write(&db);
  batch_upsert(
    &conn,
    &[street_row("Rua Alpha", 1, 0.0), street_row("Rua Beta", 2, 0.0005)],
  );
  (guard, db, index)
}

#[test]
fn _00_00_dispatches_each_named_subcommand() {
  let (_guard, db, index) = street_scene("cli_index_named");

  command_handler_index(&db, &index, Some(index_commands::admin_levels_hierarchy), &DEFAULT.index_user_friendly_name);
  command_handler_index(&db, &index, Some(index_commands::coordinates), &DEFAULT.index_user_friendly_name);
  command_handler_index(&db, &index, Some(index_commands::user_friendly_name), &DEFAULT.index_user_friendly_name);

  let conn = open_write(&db);
  assert!(
    crate::database::admin_levels_hierarchy::count(&conn) > 0,
    "the hierarchy stage must index the streets"
  );
  assert!(
    std::path::Path::new(&index).exists(),
    "the user-friendly-name stage must build the tantivy dir"
  );
}

#[test]
fn _00_01_every_leaf_returns_early_when_admin_levels_is_empty() {
  let guard = tempdir_guard::new("cli_index_empty");
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  let index = guard.path.join("idx.tantivy").to_string_lossy().into_owned();
  drop(open_write(&db));

  admin_levels_hierarchy::command_handler_index_admin_levels_hierarchy(&db);
  user_friendly_name::command_handler_index_user_friendly_name(&db, &index, &DEFAULT.index_user_friendly_name);
  coordinates::command_handler_index_coordinates(&db);
  command_handler_index(&db, &index, None, &DEFAULT.index_user_friendly_name);

  let conn = open_write(&db);
  assert_eq!(
    crate::database::admin_levels_hierarchy::count(&conn),
    0,
    "nothing must be indexed on an empty database"
  );
  assert!(
    !std::path::Path::new(&index).exists(),
    "the tantivy dir must not be created on an empty database"
  );
}
