use std::{
  io::{Read, Write},
  net::{TcpListener, TcpStream},
  path::Path,
  sync::{Arc, Mutex},
};

use super::{geofabrik, list_local, resolve_geofabrik_url};

fn start_json_server(body: String) -> String {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let port = listener.local_addr().unwrap().port();
  let url = format!("http://127.0.0.1:{port}/index.json");
  let body = Arc::new(body);
  std::thread::spawn(move || {
    for stream in listener.incoming().flatten() {
      let body = Arc::clone(&body);
      std::thread::spawn(move || serve_json(stream, &body));
    }
  });
  url
}

fn start_counting_json_server(body: String) -> (String, Arc<Mutex<u32>>) {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let port = listener.local_addr().unwrap().port();
  let url = format!("http://127.0.0.1:{port}/index.json");
  let body = Arc::new(body);
  let count: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
  let count_srv = count.clone();
  std::thread::spawn(move || {
    for stream in listener.incoming().flatten() {
      let body = Arc::clone(&body);
      let count = Arc::clone(&count_srv);
      std::thread::spawn(move || {
        *count.lock().unwrap() += 1;
        serve_json(stream, &body);
      });
    }
  });
  (url, count)
}

fn serve_json(mut stream: TcpStream, body: &str) {
  let mut buf = [0u8; 4096];
  let _ = stream.read(&mut buf);
  let response = format!(
    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
    body.len(),
    body
  );
  let _ = stream.write_all(response.as_bytes());
}

fn tmp(name: &str) -> std::path::PathBuf {
  let p = std::env::temp_dir().join(name);
  let _ = std::fs::remove_dir_all(&p);
  std::fs::create_dir_all(&p).unwrap();
  p
}

fn sqlite(name: &str) -> String {
  tmp(name).join("test.sqlite3").to_str().unwrap().to_string()
}

// 00: fetches index from http, persists rows in osm_pbf_files,
// and returns items sorted by id.
#[test]
fn _00_fetches_and_caches_when_no_cache_exists() {
  let db = sqlite("ls_t00");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"europe/germany","name":"Germany","urls":{"pbf":"http://example.com/germany.osm.pbf"}}},{"type":"Feature","properties":{"id":"europe/france","name":"France","urls":{"pbf":"http://example.com/france.osm.pbf"}}}]}"#.to_string();
  let url = start_json_server(json);
  let items = geofabrik(&db, false, &url);
  assert_eq!(items.len(), 2);
  assert_eq!(items[0].id, "europe/france");
  assert_eq!(items[0].url, "http://example.com/france.osm.pbf");
  assert_eq!(items[1].id, "europe/germany");
  assert!(Path::new(&db).exists());
}

// 01: second call with recreate_cache=false reads from sqlite, no http.
#[test]
fn _01_returns_cached_without_refetching() {
  let db = sqlite("ls_t01");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"asia/japan","name":"Japan","urls":{"pbf":"http://example.com/japan.osm.pbf"}}}]}"#.to_string();
  let (url, request_count) = start_counting_json_server(json);
  geofabrik(&db, false, &url);
  geofabrik(&db, false, &url);
  assert_eq!(*request_count.lock().unwrap(), 1);
}

// 02: recreate_cache=true forces a new http fetch even when cache exists.
#[test]
fn _02_recreate_cache_forces_refetch() {
  let db = sqlite("ls_t02");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"africa/kenya","name":"Kenya","urls":{"pbf":"http://example.com/kenya.osm.pbf"}}}]}"#.to_string();
  let (url, request_count) = start_counting_json_server(json);
  geofabrik(&db, false, &url);
  geofabrik(&db, true, &url);
  assert_eq!(*request_count.lock().unwrap(), 2);
}

// 03: feature with no pbf url appears in output with "-" as url.
#[test]
fn _03_feature_without_pbf_url_has_dash_url() {
  let db = sqlite("ls_t03");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"oceania/australia","name":"Australia"}}]}"#.to_string();
  let url = start_json_server(json);
  let items = geofabrik(&db, false, &url);
  assert_eq!(items.len(), 1);
  assert_eq!(items[0].url, "-");
}

// 04: resolve_geofabrik_url returns the pbf url for a known id.
#[test]
fn _04_resolve_geofabrik_url_returns_url_for_known_id() {
  let db = sqlite("ls_t04");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"south-america/brazil","name":"Brazil","urls":{"pbf":"http://example.com/brazil.osm.pbf"}}}]}"#.to_string();
  let url = start_json_server(json);
  let result = resolve_geofabrik_url(&db, "south-america/brazil", &url);
  assert_eq!(result.as_deref(), Some("http://example.com/brazil.osm.pbf"));
}

// 05: resolve_geofabrik_url returns none for an unknown id.
#[test]
fn _05_resolve_geofabrik_url_returns_none_for_unknown_id() {
  let db = sqlite("ls_t05");
  let json = r#"{"type":"FeatureCollection","features":[]}"#.to_string();
  let url = start_json_server(json);
  let result = resolve_geofabrik_url(&db, "nonexistent/region", &url);
  assert!(result.is_none());
}

// 06: resolve_geofabrik_url returns none when the entry has no pbf url.
#[test]
fn _06_resolve_geofabrik_url_returns_none_for_entry_without_url() {
  let db = sqlite("ls_t06");
  let json = r#"{"type":"FeatureCollection","features":[{"type":"Feature","properties":{"id":"oceania/australia","name":"Australia"}}]}"#.to_string();
  let url = start_json_server(json);
  let result = resolve_geofabrik_url(&db, "oceania/australia", &url);
  assert!(result.is_none());
}

// 07: list_local returns empty vec when pbf dir does not exist.
#[test]
fn _07_list_local_returns_empty_when_no_pbf_dir() {
  let dir = tmp("ls_t07");
  let items = list_local(dir.to_str().unwrap());
  assert!(items.is_empty());
}

// 08: list_local returns pbf files from data_path root sorted by path.
#[test]
fn _08_list_local_returns_sorted_pbf_files() {
  let dir = tmp("ls_t08");
  std::fs::write(dir.join("brazil.osm.pbf"), b"fake").unwrap();
  std::fs::write(dir.join("andorra.osm.pbf"), b"fake content longer").unwrap();
  std::fs::write(dir.join("readme.txt"), b"ignored").unwrap();
  let items = list_local(dir.to_str().unwrap());
  assert_eq!(items.len(), 2);
  assert_eq!(items[0].path.file_name().unwrap(), "andorra.osm.pbf");
  assert_eq!(items[1].path.file_name().unwrap(), "brazil.osm.pbf");
  assert_eq!(items[0].size_bytes, 19);
  assert!(Path::new(&items[0].path).exists());
}
