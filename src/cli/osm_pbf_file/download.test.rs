use super::command_handler_osm_pbf_file_download;
use crate::extract::pbf_fixtures::tempdir_guard;
use crate::osm_pbf_file::http_stubs::{md5_reply, start_file_server, start_json_server};

fn scene(tag: &str) -> (tempdir_guard, String, String) {
  let guard = tempdir_guard::new(tag);
  let data = guard.path.to_string_lossy().into_owned();
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  (guard, data, db)
}

#[test]
fn _00_00_downloads_a_url_with_two_threads_and_saves_it() {
  let (guard, data, db) = scene("cli_download_url");
  let url = start_file_server(b"fake pbf content".to_vec(), md5_reply::not_found);
  command_handler_osm_pbf_file_download(
    &data,
    &2,
    &db,
    &[url],
    "http://127.0.0.1:1/unused.json",
    false,
  );
  let saved = guard.path.join("file.osm.pbf");
  assert_eq!(
    std::fs::read(&saved).expect("downloaded file must exist"),
    b"fake pbf content"
  );
}

#[test]
fn _00_01_warns_on_md5_mismatch_but_keeps_the_file() {
  let (guard, data, db) = scene("cli_download_md5");
  let url = start_file_server(
    b"fake pbf content".to_vec(),
    md5_reply::hash("00000000000000000000000000000000".to_string()),
  );
  command_handler_osm_pbf_file_download(
    &data,
    &1,
    &db,
    &[url],
    "http://127.0.0.1:1/unused.json",
    false,
  );
  assert!(guard.path.join("file.osm.pbf").exists());
}

#[test]
fn _00_02_reuses_an_existing_file_without_downloading() {
  let (guard, data, db) = scene("cli_download_reuse");
  std::fs::write(guard.path.join("file.osm.pbf"), b"already here").expect("failed to pre-create");
  let url = start_file_server(b"other content".to_vec(), md5_reply::not_found);
  command_handler_osm_pbf_file_download(
    &data,
    &1,
    &db,
    &[url],
    "http://127.0.0.1:1/unused.json",
    false,
  );
  assert_eq!(
    std::fs::read(guard.path.join("file.osm.pbf")).expect("file must remain"),
    b"already here",
    "an existing file must be reused, not overwritten"
  );
}

#[test]
fn _00_03_resolves_a_geofabrik_id_through_the_stub_index() {
  let (guard, data, db) = scene("cli_download_resolve");
  let url = start_file_server(b"tiny region".to_vec(), md5_reply::not_found);
  let endpoint = start_json_server(format!(
    r#"{{"features":[{{"properties":{{"id":"tiny","name":"Tiny","urls":{{"pbf":"{url}"}}}}}}]}}"#
  ));
  command_handler_osm_pbf_file_download(&data, &1, &db, &["tiny".to_string()], &endpoint, false);
  assert!(guard.path.join("file.osm.pbf").exists());
}

#[test]
fn _00_04_continues_past_an_unknown_id_when_not_aborting() {
  let (guard, data, db) = scene("cli_download_continue");
  let url = start_file_server(b"fake pbf content".to_vec(), md5_reply::not_found);
  let endpoint = start_json_server(r#"{"features":[]}"#.to_string());
  command_handler_osm_pbf_file_download(
    &data,
    &1,
    &db,
    &["nope".to_string(), url],
    &endpoint,
    false,
  );
  assert!(
    guard.path.join("file.osm.pbf").exists(),
    "the second input must still be downloaded"
  );
}

#[test]
#[ignore] // executed only as a child of _01_00
fn _90_download_unknown_id_with_abort() {
  let (_guard, data, db) = scene("cli_download_abort");
  let endpoint = start_json_server(r#"{"features":[]}"#.to_string());
  command_handler_osm_pbf_file_download(&data, &1, &db, &["nope".to_string()], &endpoint, true);
}

#[test]
fn _01_00_unknown_id_with_abort_on_any_error_exits_one() {
  let out = crate::cli::tests::respawn(
    "cli::osm_pbf_file::download::tests::_90_download_unknown_id_with_abort",
    &[],
    &[],
  );
  assert_eq!(out.status.code(), Some(1), "stderr: {}", crate::cli::tests::stderr_of(&out));
  assert!(
    crate::cli::tests::stderr_of(&out)
      .contains("not a valid url nor a known geofabrik id")
  );
}
