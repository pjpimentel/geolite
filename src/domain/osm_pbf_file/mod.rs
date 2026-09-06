pub mod blob_index;
pub mod blob_scanner;
pub mod catalog;
pub mod compression;
pub mod download;
pub mod header;
pub mod http_client;
pub mod message;
pub mod repository;

pub use blob_index::{chunk_type, osm_pbf_blob_chunk};
pub use catalog::{listing, source};

pub struct osm_pbf_file<'a> {
  conn: Option<&'a rusqlite::Connection>,
  data_path: &'a str,
}

impl<'a> osm_pbf_file<'a> {
  pub fn open(conn: Option<&'a rusqlite::Connection>, data_path: &'a str) -> Self {
    osm_pbf_file { conn, data_path }
  }

  pub fn list(&self, from: source, custom_endpoint: Option<&str>, recreate_cache: bool) -> listing {
    match from {
      source::geofabrik => {
        let endpoint = endpoint_of(from, custom_endpoint);
        listing::geofabrik(catalog::geofabrik(self.database(), recreate_cache, endpoint))
      }
      source::local => listing::local(catalog::list_local(self.data_path)),
    }
  }

  pub fn resolve_geofabrik_url(&self, id: &str, custom_endpoint: Option<&str>) -> Option<String> {
    let endpoint = endpoint_of(source::geofabrik, custom_endpoint);
    catalog::resolve_geofabrik_url(self.database(), id, endpoint)
  }

  fn database(&self) -> &'a rusqlite::Connection {
    self.conn.expect("the geofabrik catalogue needs a database")
  }
}

#[cfg(test)]
#[path = "http_stubs.test.rs"]
pub(crate) mod http_stubs;

fn endpoint_of(from: source, custom_endpoint: Option<&str>) -> &str {
  custom_endpoint
    .or(from.default_endpoint())
    .expect("the geofabrik source always resolves an endpoint")
}
