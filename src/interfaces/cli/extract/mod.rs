pub mod osm_admin_levels;
pub mod osm_house_numbers;
pub mod osm_pbf_blob_chunks;
pub mod osm_pbf_data;
pub mod osm_pbf_header;

use std::io::Write;

use rusqlite::Connection;

use crate::admin_level::level;

pub(super) fn parse_name_priority(raw: &str) -> Result<Vec<&str>, String> {
  let tags: Vec<&str> = raw
    .split(',')
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .collect();
  if tags.is_empty() {
    return Err("at least one tag required".to_string());
  }
  for tag in &tags {
    if !crate::osm_tag::key::is_valid_key(tag) {
      return Err(format!("invalid characters in tag '{tag}'"));
    }
  }
  Ok(tags)
}

pub(super) fn parse_admin_levels(raw: &str) -> Result<Vec<level>, String> {
  let levels = raw
    .split(',')
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .map(|part| level::parse(part).map_err(|e| e.to_string()))
    .collect::<Result<Vec<level>, String>>()?;
  if levels.is_empty() {
    return Err("at least one level required".to_string());
  }
  Ok(levels)
}

pub(super) struct resolved_input {
  pub path: String,
  pub name: String,
  pub id: u32,
}

pub(super) fn resolve_input(
  conn: &Connection,
  data_path: &str,
  ordinal: usize,
  input: &str,
) -> Option<resolved_input> {
  if ordinal > 0 {
    println!();
  }

  let file = crate::osm_pbf_file::osm_pbf_file::open(Some(conn), data_path);
  let is_path = file.local_file(input).is_some();

  if !is_path {
    print!("\x1b[1;32mresolving\x1b[0m '{input}'...");
    let _ = std::io::stdout().flush();
  }

  let Some(path) = file.resolve(input) else {
    if !is_path {
      println!();
    }
    eprintln!("\x1b[1;31merror\x1b[0m: could not resolve '{input}'");
    return None;
  };
  if !is_path {
    println!(" done");
  }

  let name = std::path::Path::new(&path)
    .file_name()
    .unwrap_or_default()
    .to_string_lossy()
    .into_owned();
  let id = crate::osm_pbf_file::repository::ensure_by_file_path(conn, &path);
  Some(resolved_input { path, name, id })
}
