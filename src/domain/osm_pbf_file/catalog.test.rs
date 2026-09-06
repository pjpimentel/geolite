use std::path::Path;

use crate::domain::osm_pbf_file::http_stubs::{start_json_server, start_recording_json_server};
use crate::domain::osm_pbf_file::osm_pbf_file;

use super::{GEOFABRIK_ENDPOINT, geofabrik_entry, listing, local_pbf, source};

fn tmp(name: &str) -> std::path::PathBuf {
  let p = std::env::temp_dir().join(name);
  let _ = std::fs::remove_dir_all(&p);
  std::fs::create_dir_all(&p).unwrap();
  p
}

fn sqlite(name: &str) -> (String, rusqlite::Connection) {
  let path = tmp(name).join("test.sqlite3").to_str().unwrap().to_string();
  let conn = crate::database::open_write(&path);
  (path, conn)
}

fn geofabrik_entries(found: listing) -> Vec<geofabrik_entry> {
  match found {
    listing::geofabrik(entries) => entries,
    listing::local(_) => panic!("expected a geofabrik listing"),
  }
}

fn local_entries(found: listing) -> Vec<local_pbf> {
  match found {
    listing::local(entries) => entries,
    listing::geofabrik(_) => panic!("expected a local listing"),
  }
}

fn list_geofabrik(
  db: &rusqlite::Connection,
  endpoint: &str,
  recreate_cache: bool,
) -> Vec<geofabrik_entry> {
  let files = osm_pbf_file::open(Some(db), ".");
  geofabrik_entries(files.list(source::geofabrik, Some(endpoint), recreate_cache))
}

fn list_local(dir: &Path) -> Vec<local_pbf> {
  local_entries(osm_pbf_file::open(None, dir.to_str().unwrap()).list(source::local, None, false))
}

fn resolve(db: &rusqlite::Connection, id: &str, endpoint: &str) -> Option<String> {
  osm_pbf_file::open(Some(db), ".").resolve_geofabrik_url(id, Some(endpoint))
}

#[test]
fn _00_fetches_and_caches_when_no_cache_exists() {
  let (db_path, db) = sqlite("ls_t00");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"europe/germany","name":"Germany","urls":{"pbf":"http://example.com/germany.osm.pbf"}}},{"type":"Feature","properties":{"id":"europe/france","name":"France","urls":{"pbf":"http://example.com/france.osm.pbf"}}}]}"#.to_string();
  let url = start_json_server(json);
  let items = list_geofabrik(&db, &url, false);
  assert_eq!(items.len(), 2);
  assert_eq!(items[0].id, "europe/france");
  assert_eq!(items[0].url, "http://example.com/france.osm.pbf");
  assert_eq!(items[1].id, "europe/germany");
  assert!(Path::new(&db_path).exists());
}

#[test]
fn _01_returns_cached_without_refetching() {
  let (_db_path, db) = sqlite("ls_t01");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"asia/japan","name":"Japan","urls":{"pbf":"http://example.com/japan.osm.pbf"}}}]}"#.to_string();
  let (url, requests) = start_recording_json_server(json);
  list_geofabrik(&db, &url, false);
  let after_first = requests.lock().unwrap().len();
  list_geofabrik(&db, &url, false);
  assert_eq!(requests.lock().unwrap().len(), after_first);
}

#[test]
fn _02_recreate_cache_forces_refetch() {
  let (_db_path, db) = sqlite("ls_t02");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"africa/kenya","name":"Kenya","urls":{"pbf":"http://example.com/kenya.osm.pbf"}}}]}"#.to_string();
  let (url, requests) = start_recording_json_server(json);
  list_geofabrik(&db, &url, false);
  let after_first = requests.lock().unwrap().len();
  list_geofabrik(&db, &url, true);
  assert!(requests.lock().unwrap().len() > after_first);
}

#[test]
fn _03_feature_without_pbf_url_has_dash_url() {
  let (_db_path, db) = sqlite("ls_t03");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"oceania/australia","name":"Australia"}}]}"#.to_string();
  let url = start_json_server(json);
  let items = list_geofabrik(&db, &url, false);
  assert_eq!(items.len(), 1);
  assert_eq!(items[0].url, "-");
}

#[test]
fn _04_resolve_geofabrik_url_returns_url_for_known_id() {
  let (_db_path, db) = sqlite("ls_t04");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"south-america/brazil","name":"Brazil","urls":{"pbf":"http://example.com/brazil.osm.pbf"}}}]}"#.to_string();
  let url = start_json_server(json);
  let result = resolve(&db, "south-america/brazil", &url);
  assert_eq!(result.as_deref(), Some("http://example.com/brazil.osm.pbf"));
}

#[test]
fn _05_resolve_geofabrik_url_returns_none_for_unknown_id() {
  let (_db_path, db) = sqlite("ls_t05");
  let json = r#"{"type":"FeatureCollection","features":[]}"#.to_string();
  let url = start_json_server(json);
  let result = resolve(&db, "nonexistent/region", &url);
  assert!(result.is_none());
}

#[test]
fn _06_resolve_geofabrik_url_returns_none_for_entry_without_url() {
  let (_db_path, db) = sqlite("ls_t06");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"oceania/australia","name":"Australia"}}]}"#.to_string();
  let url = start_json_server(json);
  let result = resolve(&db, "oceania/australia", &url);
  assert!(result.is_none());
}

#[test]
fn _07_local_listing_is_empty_when_the_dir_has_no_pbf() {
  let dir = tmp("ls_t07");
  let items = list_local(&dir);
  assert!(items.is_empty());
}

#[test]
fn _08_local_listing_returns_sorted_pbf_files() {
  let dir = tmp("ls_t08");
  std::fs::write(dir.join("brazil.osm.pbf"), b"fake").unwrap();
  std::fs::write(dir.join("andorra.osm.pbf"), b"fake content longer").unwrap();
  std::fs::write(dir.join("readme.txt"), b"ignored").unwrap();
  let items = list_local(&dir);
  assert_eq!(items.len(), 2);
  assert_eq!(items[0].path.file_name().unwrap(), "andorra.osm.pbf");
  assert_eq!(items[1].path.file_name().unwrap(), "brazil.osm.pbf");
  assert_eq!(items[0].size_bytes, 19);
  assert!(Path::new(&items[0].path).exists());
}

#[test]
fn _09_resolve_geofabrik_url_uses_cache_without_refetching() {
  let (_db_path, db) = sqlite("ls_t09");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"europe/spain","name":"Spain","urls":{"pbf":"http://example.com/spain.osm.pbf"}}}]}"#.to_string();
  let (url, requests) = start_recording_json_server(json);
  list_geofabrik(&db, &url, false);
  let after_listing = requests.lock().unwrap().len();
  let result = resolve(&db, "europe/spain", &url);
  assert_eq!(result.as_deref(), Some("http://example.com/spain.osm.pbf"));
  assert_eq!(requests.lock().unwrap().len(), after_listing);
}

#[test]
fn _10_local_listing_is_empty_for_a_nonexistent_path() {
  let dir = tmp("ls_t10");
  let missing = dir.join("does_not_exist");
  let items = list_local(&missing);
  assert!(items.is_empty());
}

#[test]
fn _11_sends_custom_user_agent_header() {
  let (_db_path, db) = sqlite("ls_t11");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"europe/spain","name":"Spain","urls":{"pbf":"http://example.com/spain.osm.pbf"}}}]}"#.to_string();
  let (url, requests) = start_recording_json_server(json);
  list_geofabrik(&db, &url, false);
  let requests = requests.lock().unwrap();
  let user_agent = requests[0]
    .lines()
    .find(|l| l.to_lowercase().starts_with("user-agent:"))
    .and_then(|l| l.split_once(':').map(|x| x.1))
    .map(|v| v.trim().to_string());
  assert_eq!(
    user_agent,
    Some(format!("geolite/{}", env!("CARGO_PKG_VERSION")))
  );
}

#[test]
fn _12_geofabrik_resolves_its_default_endpoint_and_local_has_none() {
  assert_eq!(source::geofabrik.default_endpoint(), Some(GEOFABRIK_ENDPOINT));
  assert_eq!(
    source::geofabrik.default_endpoint(),
    Some("https://download.geofabrik.de/index-v1.json")
  );
  assert_eq!(source::local.default_endpoint(), None);
}

#[test]
fn _13_custom_endpoint_overrides_the_default() {
  let (_db_path, db) = sqlite("ls_t13");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"europe/italy","name":"Italy","urls":{"pbf":"http://example.com/italy.osm.pbf"}}}]}"#.to_string();
  let (url, requests) = start_recording_json_server(json);
  let items = list_geofabrik(&db, &url, false);
  assert_eq!(items.len(), 1);
  assert_eq!(requests.lock().unwrap().len(), 1);
}

#[test]
fn _14_local_listing_needs_no_database() {
  let dir = tmp("ls_t14");
  std::fs::write(dir.join("andorra.osm.pbf"), b"fake").unwrap();
  let items = local_entries(osm_pbf_file::open(None, dir.to_str().unwrap()).list(
    source::local,
    Some("http://127.0.0.1:1/unused.json"),
    true,
  ));
  assert_eq!(items.len(), 1);
  assert!(!dir.join("test.sqlite3").exists());
}

#[test]
#[should_panic(expected = "only `ls local` runs without a database")]
fn _15_geofabrik_listing_without_a_database_panics() {
  let dir = tmp("ls_t15");
  osm_pbf_file::open(None, dir.to_str().unwrap()).list(
    source::geofabrik,
    Some("http://127.0.0.1:1/unused.json"),
    false,
  );
}
