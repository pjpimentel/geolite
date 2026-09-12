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

#[test]
fn _04_delete_removes_the_file_its_chunks_and_forgets_the_download() {
  let scene = temp_scene("facade_04");
  write_pbf(&scene.pbf_path, &tiny_pbf());
  let data_path = data_dir(&scene);
  let conn = crate::database::open_write(&scene.db_path);
  let file = osm_pbf_file::open(Some(&conn), &data_path);
  file.extract_blob_chunks(&scene.pbf_path, |_| {});
  let file_id = repository::ensure_by_file_path(&conn, &scene.pbf_path);
  let size = std::fs::metadata(&scene.pbf_path)
    .expect("the pbf exists")
    .len();

  let bytes = file.delete(&scene.pbf_path);

  assert_eq!(bytes, size, "the bytes freed are the file's size");
  assert!(!std::path::Path::new(&scene.pbf_path).exists());
  assert_eq!(blob_index::count_by_file_id(&conn, file_id), 0);
  assert_eq!(
    repository::get_file_path(&conn, &file_id.to_string()),
    None,
    "a deleted file resolves to nothing"
  );
  assert_eq!(file.delete(&scene.pbf_path), 0, "deleting twice frees nothing");
}

#[test]
fn _05_resolve_answers_a_path_a_name_under_data_path_or_a_ledger_id() {
  let scene = temp_scene("facade_05");
  write_pbf(&scene.pbf_path, &tiny_pbf());
  let data_path = data_dir(&scene);
  let conn = crate::database::open_write(&scene.db_path);
  let file = osm_pbf_file::open(Some(&conn), &data_path);
  let name = std::path::Path::new(&scene.pbf_path)
    .file_name()
    .and_then(|n| n.to_str())
    .expect("the scene names its pbf")
    .to_string();
  let file_id = repository::ensure_by_file_path(&conn, &scene.pbf_path);

  assert_eq!(file.resolve(&scene.pbf_path).as_deref(), Some(scene.pbf_path.as_str()));
  assert_eq!(
    file.resolve(&name).as_deref(),
    Some(std::path::Path::new(&data_path).join(&name).to_string_lossy().as_ref()),
    "a bare name is looked up under data_path"
  );
  assert_eq!(
    file.resolve(&file_id.to_string()).as_deref(),
    Some(scene.pbf_path.as_str()),
    "an id is looked up in the ledger"
  );
  assert_eq!(file.resolve("missing"), None);
  assert!(
    osm_pbf_file::open(None, &data_path).local_file(&name).is_some(),
    "a local file needs no database"
  );
}

#[test]
fn _06_download_saves_the_file_and_records_it_in_the_ledger() {
  use crate::domain::osm_pbf_file::http_stubs::{md5_reply, start_file_server};

  let scene = temp_scene("facade_06");
  let data_path = data_dir(&scene);
  let conn = crate::database::open_write(&scene.db_path);
  let file = osm_pbf_file::open(Some(&conn), &data_path);
  let content = b"fake pbf content".to_vec();
  let url = start_file_server(content.clone(), md5_reply::not_found);
  let from = repository::origin::url(url.clone());

  let output = file.download(&from, 1, |_| {});

  let saved = std::path::Path::new(&data_path).join(from.file_name());
  assert_eq!(output.path, saved);
  assert_eq!(std::fs::read(&saved).expect("the file was saved"), content);
  assert_eq!(
    repository::get_file_path(&conn, &url).as_deref(),
    Some(saved.to_string_lossy().as_ref()),
    "the ledger resolves the url to the saved file"
  );
  assert_eq!(
    column::<i64>(&conn, &saved.to_string_lossy(), "size_bytes"),
    Some(content.len() as i64)
  );
}
