use std::{fs, path::PathBuf};

use rusqlite::Connection;
use serde::Deserialize;

use super::{http_client, repository};

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

pub const GEOFABRIK_ENDPOINT: &str = "https://download.geofabrik.de/index-v1.json";

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum source {
  geofabrik,
  local,
}

impl source {
  pub fn default_endpoint(self) -> Option<&'static str> {
    match self {
      source::geofabrik => Some(GEOFABRIK_ENDPOINT),
      source::local => None,
    }
  }
}

pub struct geofabrik_entry {
  pub id: String,
  pub name: String,
  pub url: String,
}

pub struct local_pbf {
  pub path: PathBuf,
  pub size_bytes: u64,
}

pub enum listing {
  geofabrik(Vec<geofabrik_entry>),
  local(Vec<local_pbf>),
}

pub(super) fn geofabrik(
  conn: &Connection,
  recreate_cache: bool,
  endpoint: &str,
) -> Vec<geofabrik_entry> {
  if !recreate_cache {
    let cached = repository::list_geofabrik_index(conn);
    if !cached.is_empty() {
      return cached
        .into_iter()
        .map(|(id, name, url)| geofabrik_entry { id, name, url })
        .collect();
    }
  }
  let body = http_client::agent()
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
    repository::upsert_geofabrik_index_item(
      &tx,
      &f.properties.id,
      &f.properties.name,
      f.properties.parent.as_deref(),
      url,
    );
  }
  tx.commit().expect("failed to commit transaction");
  repository::list_geofabrik_index(conn)
    .into_iter()
    .map(|(id, name, url)| geofabrik_entry { id, name, url })
    .collect()
}

pub(super) fn resolve_geofabrik_url(conn: &Connection, id: &str, endpoint: &str) -> Option<String> {
  if let Some(url) = repository::get_geofabrik_url(conn, id)
    && url != "-"
  {
    return Some(url);
  }
  geofabrik(conn, false, endpoint)
    .into_iter()
    .find(|i| i.id == id)
    .and_then(|i| if i.url == "-" { None } else { Some(i.url) })
}

pub(super) fn list_local(data_path: &str) -> Vec<local_pbf> {
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
#[path = "catalog.test.rs"]
mod tests;
