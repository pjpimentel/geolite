use crate::database::merge_fixtures::{
  build_source, cleanup, count, make_house, make_way, temp_path,
};
use crate::database::open_write_main;
use crate::domain::admin_level::id::admin_level_id;

#[test]
fn _00_merge_combines_admin_levels_without_id_collision() {
  let base_path = temp_path("admins_base");
  let source_path = temp_path("admins_source");

  // base covers ways {1, 2}; source covers ways {2, 3} — way 2 overlaps.
  build_source(&base_path, &[make_way(1), make_way(2)], &[]);
  build_source(&source_path, &[make_way(2), make_way(3)], &[]);

  let conn = open_write_main(&base_path);
  let (admins, _) = super::merge_source(&conn, &source_path);
  assert_eq!(admins, 2, "source contributed two admin_levels rows");

  // ways {1, 2, 3} → exactly three rows; way 2 upserted, not duplicated.
  assert_eq!(count(&conn, "SELECT COUNT(*) FROM admin_levels"), 3);

  let way3_id = admin_level_id::from_way(3).raw() as i64;
  assert_eq!(
    count(&conn, &format!("SELECT COUNT(*) FROM admin_levels WHERE id = {way3_id}")),
    1,
    "way 3 from the source is present after merge",
  );

  drop(conn);
  cleanup(&base_path);
  cleanup(&source_path);
}

#[test]
fn _01_merge_house_numbers_dedupes_by_node_id() {
  let base_path = temp_path("houses_base");
  let source_path = temp_path("houses_source");

  // both databases reference admin_level way 1 (id = 2). node 100 overlaps; node 200 is new.
  let way1_id = admin_level_id::from_way(1).raw() as i64;
  build_source(&base_path, &[make_way(1)], &[make_house(100, way1_id, "10")]);
  build_source(
    &source_path,
    &[make_way(1)],
    &[make_house(100, way1_id, "999"), make_house(200, way1_id, "20")],
  );

  let conn = open_write_main(&base_path);
  super::merge_source(&conn, &source_path);

  // nodes {100, 200} → exactly two rows; node 100 deduped by its UNIQUE constraint.
  assert_eq!(count(&conn, "SELECT COUNT(*) FROM house_numbers"), 2);

  // INSERT OR IGNORE keeps the base row for node 100 (number "10", not the source's "999").
  let number: String = conn
    .query_row(
      "SELECT number FROM house_numbers WHERE node_id = 100",
      [],
      |row| row.get(0),
    )
    .expect("node 100 must exist");
  assert_eq!(number, "10");

  drop(conn);
  cleanup(&base_path);
  cleanup(&source_path);
}
