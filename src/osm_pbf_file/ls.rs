use std::{fs, path::PathBuf};

use serde::Deserialize;

use crate::database;

#[derive(Deserialize)]
struct geofabrik_index {
  features: Vec<geofabrik_feature>,
}

#[derive(Deserialize)]
struct geofabrik_feature {
  properties: geofabrik_properties,
}

#[derive(Deserialize)]
struct geofabrik_properties {
  id: String,
  name: String,
  #[serde(default)]
  parent: Option<String>,
  #[serde(default)]
  urls: Option<geofabrik_urls>,
}

#[derive(Deserialize)]
struct geofabrik_urls {
  pbf: Option<String>,
}

pub struct ls_output_item {
  pub id: String,
  pub name: String,
  pub url: String,
}

pub struct local_pbf {
  pub path: PathBuf,
  pub size_bytes: u64,
}

pub fn geofabrik(sqlite_path: &str, recreate_cache: bool, endpoint: &str) -> Vec<ls_output_item> {
  let conn = database::open_write(sqlite_path);
  if !recreate_cache {
    let cached = database::osm_pbf_files::list_geofabrik_index(&conn);
    if !cached.is_empty() {
      return cached
        .into_iter()
        .map(|(id, name, url)| ls_output_item { id, name, url })
        .collect();
    }
  }
  let body = super::agent()
    .get(endpoint)
    .call()
    .expect("failed to fetch geofabrik index")
    .into_body()
    .read_to_string()
    .expect("failed to read response body");
  let index: geofabrik_index =
    serde_json::from_str(&body).expect("failed to parse geofabrik index");
  let tx = conn
    .unchecked_transaction()
    .expect("failed to begin transaction");
  for f in &index.features {
    let url = f
      .properties
      .urls
      .as_ref()
      .and_then(|u| u.pbf.as_deref())
      .unwrap_or("-");
    database::osm_pbf_files::upsert_geofabrik_index_item(
      &tx,
      &f.properties.id,
      &f.properties.name,
      f.properties.parent.as_deref(),
      url,
    );
  }
  tx.commit().expect("failed to commit transaction");
  database::osm_pbf_files::list_geofabrik_index(&conn)
    .into_iter()
    .map(|(id, name, url)| ls_output_item { id, name, url })
    .collect()
}

pub fn resolve_geofabrik_url(sqlite_path: &str, id: &str, endpoint: &str) -> Option<String> {
  let conn = database::open_write(sqlite_path);
  if let Some(url) = database::osm_pbf_files::get_geofabrik_url(&conn, id)
    && url != "-"
  {
    return Some(url);
  }
  drop(conn);
  geofabrik(sqlite_path, false, endpoint)
    .into_iter()
    .find(|i| i.id == id)
    .and_then(|i| if i.url == "-" { None } else { Some(i.url) })
}

pub fn list_local(data_path: &str) -> Vec<local_pbf> {
  let pbf_dir = PathBuf::from(data_path);
  let Ok(entries) = fs::read_dir(&pbf_dir) else {
    return vec![];
  };
  let mut result: Vec<local_pbf> = entries
    .flatten()
    .filter(|e| {
      e.path()
        .extension()
        .map(|ext| ext == "pbf")
        .unwrap_or(false)
    })
    .filter_map(|e| {
      let size_bytes = e.metadata().ok()?.len();
      Some(local_pbf {
        path: e.path(),
        size_bytes,
      })
    })
    .collect();
  result.sort_by(|a, b| a.path.cmp(&b.path));
  result
}

#[cfg(test)]
#[path = "ls.test.rs"]
mod ls_test;
