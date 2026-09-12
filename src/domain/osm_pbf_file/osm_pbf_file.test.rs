use super::{blob_index, data_opts, osm_pbf_file, repository};

use crate::domain::osm_tag::tag_policy;
use crate::domain::pbf_fixtures::{header_chunk, indexed_scene, temp_scene, tiny_pbf, write_pbf};

fn data_dir(scene: &temp_scene) -> String {
  scene.guard.path.to_string_lossy().into_owned()
}

fn every_element() -> data_opts {
  data_opts {
    include_nodes: true,
    include_ways: true,
    include_relations: true,
    ignore_info: true,
    tags: tag_policy::default(),
    buffer_bytes: 8 * 1024 * 1024,
  }
}

fn column<T: rusqlite::types::FromSql>(
  conn: &rusqlite::Connection,
  path: &str,
  name: &str,
) -> Option<T> {
  conn
    .query_row(
      &format!("SELECT {name} FROM osm_pbf_files WHERE path = ?1"),
      rusqlite::params![path],
      |row| row.get::<_, Option<T>>(0),
    )
    .expect("failed to read osm_pbf_files")
}

#[test]
fn _00_extract_blob_chunks_indexes_every_blob_of_the_file() {
  let scene = temp_scene("facade_00");
  let chunks = tiny_pbf();
  write_pbf(&scene.pbf_path, &chunks);
  let data_path = data_dir(&scene);
  let conn = crate::database::open_write(&scene.db_path);
  let file = osm_pbf_file::open(Some(&conn), &data_path);

  let count = file.extract_blob_chunks(&scene.pbf_path, |_| {});

  assert_eq!(count, chunks.len());
  let file_id = repository::ensure_by_file_path(&conn, &scene.pbf_path);
  assert_eq!(
    blob_index::count_by_file_id(&conn, file_id),
    chunks.len() as i64,
    "every blob of the file must be indexed"
  );
}

#[test]
fn _01_extract_osm_header_fills_the_header_columns() {
  let (scene, _file_id) = indexed_scene("facade_01", &[header_chunk()]);
  let data_path = data_dir(&scene);
  let conn = crate::database::open_write(&scene.db_path);
  let file = osm_pbf_file::open(Some(&conn), &data_path);

  let header = file.extract_osm_header(&scene.pbf_path);

  assert_eq!(header.writingprogram.as_deref(), Some("geolite-test"));
  assert_eq!(
    column::<String>(&conn, &scene.pbf_path, "osm_header_writingprogram").as_deref(),
    Some("geolite-test")
  );
}

#[test]
fn _02_extract_osm_data_fills_the_tables_and_the_counts() {
  let (scene, _file_id) = indexed_scene("facade_02", &tiny_pbf());
  let data_path = data_dir(&scene);
  let conn = crate::database::open_write(&scene.db_path);
  let file = osm_pbf_file::open(Some(&conn), &data_path);

  let counts = file
    .extract_osm_data(
      &scene.pbf_path,
      crate::database::open_write(&scene.db_path),
      every_element(),
      1,
      |_| {},
    )
    .expect("the fixture has indexed data chunks");

  assert_eq!((counts.nodes, counts.ways, counts.relations), (1, 1, 1));
  let ways: i64 = conn
    .query_row("SELECT COUNT(*) FROM osm_data.osm_ways", [], |row| row.get(0))
    .expect("failed to count osm_ways");
  assert_eq!(ways, 1);
  assert_eq!(column::<i64>(&conn, &scene.pbf_path, "node_count"), Some(1));
  assert_eq!(column::<i64>(&conn, &scene.pbf_path, "relation_count"), Some(1));
  assert!(column::<i64>(&conn, &scene.pbf_path, "osm_data_extracted_at").is_some());
}

#[test]
fn _03_extract_osm_data_returns_none_without_blob_chunks() {
  let scene = temp_scene("facade_03");
  write_pbf(&scene.pbf_path, &tiny_pbf());
  let data_path = data_dir(&scene);
  let conn = crate::database::open_write(&scene.db_path);
  let file = osm_pbf_file::open(Some(&conn), &data_path);

  let counts = file.extract_osm_data(
    &scene.pbf_path,
    crate::database::open_write(&scene.db_path),
    every_element(),
    1,
    |_| {},
  );

  assert!(counts.is_none(), "nothing is extracted from a file that was never scanned");
  assert!(
    column::<i64>(&conn, &scene.pbf_path, "osm_data_extracted_at").is_none(),
    "the ledger must not be stamped"
  );
}
