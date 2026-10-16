pub mod blob_index;
pub mod blob_scanner;
pub mod catalog;
pub mod compression;
pub mod download;
pub mod header;
pub mod http_client;
pub mod message;
pub mod osm_data;
pub mod repository;

use std::collections::BTreeMap;

use crate::database::table;

pub use blob_index::osm_pbf_blob_chunks;
pub use catalog::{input_kind, listing, source};
pub use osm_data::data_opts;
pub use repository::{origin, osm_pbf_files};

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

  pub fn local_file(&self, input: &str) -> Option<String> {
    if std::path::Path::new(input).exists() {
      return Some(input.to_string());
    }
    let in_data = std::path::Path::new(self.data_path).join(input);
    in_data
      .exists()
      .then(|| in_data.to_string_lossy().into_owned())
  }

  pub fn resolve(&self, input: &str) -> Option<String> {
    self
      .local_file(input)
      .or_else(|| repository::get_file_path(self.database(), input))
  }

  pub fn extract_blob_chunks(
    &self,
    path: &str,
    on_progress: impl Fn(blob_scanner::progress),
  ) -> usize {
    let file_id = repository::ensure_by_file_path(self.database(), path);
    blob_scanner::run(path, self.database(), file_id, on_progress)
  }

  pub fn extract_osm_header(&self, path: &str) -> header::header_output {
    let file_id = repository::ensure_by_file_path(self.database(), path);
    header::run(path, self.database(), file_id)
  }

  pub fn extract_osm_data(
    &self,
    path: &str,
    write_conn: rusqlite::Connection,
    opts: data_opts,
    threads: u8,
    on_progress: impl Fn(osm_data::progress) + Send + 'static,
  ) -> Option<osm_data::osm_data_counts> {
    let conn = self.database();
    let file_id = repository::ensure_by_file_path(conn, path);
    let chunks = blob_index::get_data_chunks(conn, file_id);
    if chunks.is_empty() {
      return None;
    }
    let (nodes, ways, relations) =
      osm_data::run(path, chunks, write_conn, opts, &threads, on_progress);
    repository::update_counts(conn, file_id, nodes as u64, ways as u64, relations as u64);
    Some(osm_data::osm_data_counts {
      nodes,
      ways,
      relations,
    })
  }

  pub fn download(
    &self,
    from: &origin,
    threads: u8,
    on_event: impl Fn(download::download_event) + Send + Sync + 'static,
  ) -> download::download_output {
    let url = from.download_url().expect("a download origin has a url");
    let output = download::run(self.data_path, url, threads, on_event)
      .expect("download produced no output");
    let conn = self.database();
    osm_pbf_files::create_indexes(conn);
    repository::update_downloaded(
      conn,
      from,
      output.path.to_str().unwrap_or(""),
      output.total_bytes,
      &output.actual_md5,
    );
    output
  }

  // the ledger counts what every base holds of every file whose chunks are in the osm_data: one
  // base attached at a time, the counts per file summed across the bases, and 0 stamped on a file
  // the bases hold nothing of, so it still reads as counted
  pub fn stamp_admin_levels_count(&self, bases: &[String]) {
    let totals = self.summed_per_file(bases, repository::count_admin_levels_per_file);
    for file_id in blob_index::file_ids(self.database()) {
      let total = totals.get(&file_id).copied().unwrap_or(0);
      repository::set_admin_levels_count(self.database(), file_id, total);
    }
  }

  pub fn stamp_house_numbers_count(&self, bases: &[String]) {
    let totals = self.summed_per_file(bases, repository::count_house_numbers_per_file);
    for file_id in blob_index::file_ids(self.database()) {
      let total = totals.get(&file_id).copied().unwrap_or(0);
      repository::set_house_numbers_count(self.database(), file_id, total);
    }
  }

  fn summed_per_file(
    &self,
    bases: &[String],
    count: impl Fn(&rusqlite::Connection) -> Vec<(u32, i64)>,
  ) -> BTreeMap<u32, i64> {
    let conn = self.database();
    let mut totals: BTreeMap<u32, i64> = BTreeMap::new();
    for base in bases {
      crate::database::store::attach_base(conn, base);
      for (file_id, rows) in count(conn) {
        *totals.entry(file_id).or_insert(0) += rows;
      }
      crate::database::store::detach_base(conn);
    }
    totals
  }

  pub fn delete(&self, path: &str) -> u64 {
    let conn = self.database();
    if let Some(file_id) = repository::get_id_by_path(conn, path) {
      blob_index::delete_by_file_id(conn, file_id);
      repository::clear_download(conn, path);
    }
    let file = std::path::Path::new(path);
    if !file.exists() {
      return 0;
    }
    let bytes = std::fs::metadata(file).map(|m| m.len()).unwrap_or(0);
    std::fs::remove_file(file).expect("failed to remove osm.pbf file");
    bytes
  }

  fn database(&self) -> &'a rusqlite::Connection {
    self.conn.expect("only `ls local` runs without a database")
  }
}



fn endpoint_of(from: source, custom_endpoint: Option<&str>) -> &str {
  custom_endpoint
    .or(from.default_endpoint())
    .expect("the geofabrik source always resolves an endpoint")
}
