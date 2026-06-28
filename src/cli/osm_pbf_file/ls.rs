use super::osm_pbf_file_ls_source;

pub fn command_handler_osm_pbf_file_ls(
  data_path: &str,
  sqlite_path: &str,
  source: &osm_pbf_file_ls_source,
  ls_endpoint: &str,
  recreate_cache: &bool,
) {
  match source {
    osm_pbf_file_ls_source::geofabrik => {
      let items = crate::osm_pbf_file::ls::geofabrik(sqlite_path, *recreate_cache, ls_endpoint);
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
    osm_pbf_file_ls_source::local => {
      let files = crate::osm_pbf_file::ls::list_local(data_path);
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
