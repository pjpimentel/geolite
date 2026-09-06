use rusqlite::Connection;

use super::{
  ensure_by_file_path, get_file_path, list_geofabrik_index, origin, update_downloaded,
  upsert_geofabrik_index_item,
};

fn database(name: &str) -> Connection {
  let dir = std::env::temp_dir().join(format!("osm_pbf_files_{name}"));
  let _ = std::fs::remove_dir_all(&dir);
  std::fs::create_dir_all(&dir).unwrap();
  crate::database::open_write(dir.join("db.sqlite3").to_str().unwrap())
}

type row = (u8, Option<String>, Option<String>, Option<String>, Option<String>);

fn row_by_path(conn: &Connection, path: &str) -> row {
  conn
    .query_row(
      "SELECT origin, origin_id, origin_name, url, path FROM osm_pbf_files WHERE path = ?1",
      [path],
      |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )
    .expect("row by path")
}

fn row_count(conn: &Connection) -> i64 {
  conn
    .query_row("SELECT COUNT(*) FROM osm_pbf_files", [], |r| r.get(0))
    .expect("count")
}

#[test]
fn _00_origin_codes_and_download_urls_are_stable() {
  let local = origin::local_path("./data/santos.osm.pbf".into());
  let catalogue = origin::geofabrik {
    id: "south-america/brazil".into(),
    url: "https://download.geofabrik.de/south-america/brazil-latest.osm.pbf".into(),
  };
  let direct = origin::url("http://mirror.invalid/alpha.osm.pbf".into());
  assert_eq!(local.code(), 0);
  assert_eq!(catalogue.code(), 1);
  assert_eq!(direct.code(), 2);
  assert_eq!(local.download_url(), None);
  assert_eq!(
    catalogue.download_url(),
    Some("https://download.geofabrik.de/south-america/brazil-latest.osm.pbf")
  );
  assert_eq!(direct.download_url(), Some("http://mirror.invalid/alpha.osm.pbf"));
}

#[test]
fn _01_ensure_by_file_path_records_a_local_file_once_with_its_name() {
  let conn = database("t01");
  let path = "/data/extracts/santos.osm.pbf";
  let first = ensure_by_file_path(&conn, path);
  let second = ensure_by_file_path(&conn, path);
  assert_eq!(first, second);
  assert_eq!(row_count(&conn), 1);
  let (code, id, name, url, stored) = row_by_path(&conn, path);
  assert_eq!(code, 0);
  assert_eq!(id, None);
  assert_eq!(name.as_deref(), Some("santos.osm.pbf"));
  assert_eq!(url, None);
  assert_eq!(stored.as_deref(), Some(path));
}

#[test]
fn _02_ensure_by_file_path_keeps_the_origin_of_a_downloaded_file() {
  let conn = database("t02");
  let path = "/data/alpha.osm.pbf";
  let from = origin::url("http://mirror.invalid/alpha.osm.pbf".into());
  update_downloaded(&conn, &from, path, 65536, "d41d8cd98f00b204e9800998ecf8427e");
  ensure_by_file_path(&conn, path);
  assert_eq!(row_count(&conn), 1);
  let (code, _, name, url, _) = row_by_path(&conn, path);
  assert_eq!(code, 2);
  assert_eq!(name.as_deref(), Some("alpha.osm.pbf"));
  assert_eq!(url.as_deref(), Some("http://mirror.invalid/alpha.osm.pbf"));
}

#[test]
fn _03_a_url_download_is_promoted_when_the_catalogue_lists_its_url() {
  let conn = database("t03");
  let url = "https://download.geofabrik.de/europe/andorra-latest.osm.pbf";
  let path = "/data/andorra-latest.osm.pbf";
  update_downloaded(&conn, &origin::url(url.into()), path, 10, "md5");
  upsert_geofabrik_index_item(&conn, "europe/andorra", "Andorra", url);
  assert_eq!(row_count(&conn), 1);
  let (code, id, name, stored_url, stored) = row_by_path(&conn, path);
  assert_eq!(code, 1);
  assert_eq!(id.as_deref(), Some("europe/andorra"));
  assert_eq!(name.as_deref(), Some("Andorra"));
  assert_eq!(stored_url.as_deref(), Some(url));
  assert_eq!(stored.as_deref(), Some(path));
  assert_eq!(
    list_geofabrik_index(&conn),
    vec![("europe/andorra".into(), "Andorra".into(), url.into())]
  );
}

#[test]
fn _04_a_geofabrik_download_fills_the_catalogue_row() {
  let conn = database("t04");
  let url = "https://download.geofabrik.de/europe/andorra-latest.osm.pbf";
  upsert_geofabrik_index_item(&conn, "europe/andorra", "Andorra", url);
  let from = origin::geofabrik {
    id: "europe/andorra".into(),
    url: url.into(),
  };
  update_downloaded(&conn, &from, "/data/andorra-latest.osm.pbf", 10, "md5");
  assert_eq!(row_count(&conn), 1);
  let (code, id, name, _, stored) = row_by_path(&conn, "/data/andorra-latest.osm.pbf");
  assert_eq!(code, 1);
  assert_eq!(id.as_deref(), Some("europe/andorra"));
  assert_eq!(name.as_deref(), Some("Andorra"));
  assert_eq!(stored.as_deref(), Some("/data/andorra-latest.osm.pbf"));
}

#[test]
fn _05_a_geofabrik_download_without_a_catalogue_row_inserts_one() {
  let conn = database("t05");
  let url = "https://download.geofabrik.de/europe/andorra-latest.osm.pbf";
  let from = origin::geofabrik {
    id: "europe/andorra".into(),
    url: url.into(),
  };
  update_downloaded(&conn, &from, "/data/andorra-latest.osm.pbf", 10, "md5");
  let (code, id, name, _, _) = row_by_path(&conn, "/data/andorra-latest.osm.pbf");
  assert_eq!(code, 1);
  assert_eq!(id.as_deref(), Some("europe/andorra"));
  assert_eq!(name.as_deref(), Some("andorra-latest.osm.pbf"));
  upsert_geofabrik_index_item(&conn, "europe/andorra", "Andorra", url);
  assert_eq!(row_count(&conn), 1);
  let (_, _, name, _, _) = row_by_path(&conn, "/data/andorra-latest.osm.pbf");
  assert_eq!(name.as_deref(), Some("Andorra"));
}

#[test]
fn _06_get_file_path_resolves_a_geofabrik_id_and_a_numeric_id() {
  let conn = database("t06");
  let url = "https://download.geofabrik.de/europe/andorra-latest.osm.pbf";
  let from = origin::geofabrik {
    id: "europe/andorra".into(),
    url: url.into(),
  };
  update_downloaded(&conn, &from, "/data/andorra-latest.osm.pbf", 10, "md5");
  let local = ensure_by_file_path(&conn, "/data/santos.osm.pbf");
  assert_eq!(
    get_file_path(&conn, "europe/andorra").as_deref(),
    Some("/data/andorra-latest.osm.pbf")
  );
  assert_eq!(
    get_file_path(&conn, &local.to_string()).as_deref(),
    Some("/data/santos.osm.pbf")
  );
  assert_eq!(get_file_path(&conn, "europe/nowhere"), None);
}

#[test]
fn _07_list_geofabrik_index_ignores_local_and_url_rows() {
  let conn = database("t07");
  ensure_by_file_path(&conn, "/data/santos.osm.pbf");
  update_downloaded(
    &conn,
    &origin::url("http://mirror.invalid/alpha.osm.pbf".into()),
    "/data/alpha.osm.pbf",
    10,
    "md5",
  );
  upsert_geofabrik_index_item(&conn, "europe/andorra", "Andorra", "https://example.com/andorra.osm.pbf");
  assert_eq!(row_count(&conn), 3);
  assert_eq!(list_geofabrik_index(&conn).len(), 1);
}

#[test]
fn _08_a_file_extracted_before_being_downloaded_takes_the_download_origin() {
  let conn = database("t08");
  let path = "/data/alpha.osm.pbf";
  let local = ensure_by_file_path(&conn, path);
  update_downloaded(
    &conn,
    &origin::url("http://mirror.invalid/alpha.osm.pbf".into()),
    path,
    65536,
    "md5",
  );
  assert_eq!(row_count(&conn), 1);
  assert_eq!(ensure_by_file_path(&conn, path), local);
  let (code, _, _, url, _) = row_by_path(&conn, path);
  assert_eq!(code, 2);
  assert_eq!(url.as_deref(), Some("http://mirror.invalid/alpha.osm.pbf"));
}

#[test]
#[should_panic(expected = "never recorded as downloaded")]
fn _09_a_local_path_is_never_recorded_as_downloaded() {
  let conn = database("t09");
  update_downloaded(
    &conn,
    &origin::local_path("/data/santos.osm.pbf".into()),
    "/data/santos.osm.pbf",
    10,
    "md5",
  );
}
