use super::osm_pbf_file_ls_source;
use crate::domain::osm_pbf_file::{listing, osm_pbf_file};

pub fn command_handler_osm_pbf_file_ls(
  data_path: &str,
  sqlite_path: &str,
  from: &osm_pbf_file_ls_source,
  ls_endpoint: Option<&str>,
  recreate_cache: &bool,
) {
  let found = match from {
    osm_pbf_file_ls_source::geofabrik => {
      let conn = crate::database::open_write(sqlite_path);
      osm_pbf_file::open(Some(&conn), data_path).list((*from).into(), ls_endpoint, *recreate_cache)
    }
    osm_pbf_file_ls_source::local => {
      osm_pbf_file::open(None, data_path).list((*from).into(), ls_endpoint, *recreate_cache)
    }
  };
  match found {
    listing::geofabrik(items) => {
      let id_w = items.iter().map(|i| i.id.len()).max().unwrap_or(0).max(2);
      let name_w = items.iter().map(|i| i.name.len()).max().unwrap_or(0).max(4);
      let url_w = items.iter().map(|i| i.url.len()).max().unwrap_or(0).max(3);
      println!("{:<id_w$}  {:<name_w$}  {:<url_w$}", "id", "name", "url");
      println!("{:-<id_w$}  {:-<name_w$}  {:-<url_w$}", "", "", "");
      for item in items {
        println!(
          "{:<id_w$}  {:<name_w$}  {:<url_w$}",
          item.id, item.name, item.url
        );
      }
    }
    listing::local(files) => {
      if files.is_empty() {
        println!("no pbf files found in {data_path}/");
        return;
      }
      let path_w = files
        .iter()
        .map(|f| f.path.display().to_string().len())
        .max()
        .unwrap_or(0)
        .max(4);
      println!("{:<path_w$}  size", "path");
      println!("{:-<path_w$}  ----", "");
      for f in files {
        println!("{:<path_w$}  {} bytes", f.path.display(), f.size_bytes);
      }
    }
  }
}

#[cfg(test)]
#[path = "ls.test.rs"]
mod tests;
