use std::collections::HashSet;

use crate::database::store::{self, base_files};
use crate::interfaces::cli::extract::{
  osm_admin_levels::command_handler_extract_osm_admin_levels,
  osm_house_numbers::command_handler_extract_osm_house_numbers,
  osm_pbf_blob_chunks::command_handler_extract_osm_pbf_blob_chunks,
  osm_pbf_data::command_handler_extract_osm_pbf_data,
  osm_pbf_header::command_handler_extract_osm_pbf_header,
};
use crate::interfaces::cli::exec::stages::stage;
use crate::interfaces::cli::index::index_bases;
use crate::interfaces::cli::optimize::command_handler_optimize;
use crate::interfaces::cli::osm_pbf_file::download::command_handler_osm_pbf_file_download;
use crate::osm_pbf_file::{input_kind, osm_pbf_file};

pub fn command_handler_build(
  data_path: &str,
  threads: &u8,
  sources: &[String],
  ls_endpoint: Option<&str>,
  abort_on_any_error: bool,
) {
  // filesystem only, on purpose: the catalogue may not exist yet at this point of the build, and
  // a local source must never reach resolve_geofabrik_url, which fetches the geofabrik index with
  // an `expect` that would take the whole pipeline down when the network fails
  let files = osm_pbf_file::open(None, data_path);
  let local: Vec<bool> = sources
    .iter()
    .map(|source| files.local_file(source).is_some())
    .collect();
  for (source, &is_local) in sources.iter().zip(&local) {
    if input_kind::of(source) == input_kind::local_path && !is_local {
      eprintln!("\x1b[1;31merror\x1b[0m: source file not found: {source}");
      std::process::exit(1);
    }
  }

  let inputs: Vec<String> = sources.to_vec();
  let remote: Vec<String> = sources
    .iter()
    .zip(&local)
    .filter(|(_, is_local)| !**is_local)
    .map(|(source, _)| source.clone())
    .collect();

  println!("\x1b[2m── download\x1b[0m");
  for (source, &is_local) in sources.iter().zip(&local) {
    if is_local {
      println!("\x1b[1;33mskipping\x1b[0m download — '{source}' is already a local file");
    }
  }
  if !remote.is_empty() {
    command_handler_osm_pbf_file_download(
      data_path,
      threads,
      &remote,
      ls_endpoint,
      abort_on_any_error,
    );
    let conn = store::open_catalog(data_path);
    let files = osm_pbf_file::open(Some(&conn), data_path);
    for source in &remote {
      if files.resolve(source).is_none() {
        eprintln!(
          "\x1b[1;31merror\x1b[0m: could not resolve '{source}' after download — aborting pipeline"
        );
        std::process::exit(1);
      }
    }
  }

  println!();
  println!("\x1b[2m── {}\x1b[0m", stage::extract_osm_pbf_blob_chunks.name());
  command_handler_extract_osm_pbf_blob_chunks(data_path, &inputs, false);

  println!();
  println!("\x1b[2m── {}\x1b[0m", stage::extract_osm_pbf_header.name());
  command_handler_extract_osm_pbf_header(data_path, &inputs);

  println!();
  println!("\x1b[2m── {}\x1b[0m", stage::extract_osm_pbf_data.name());
  command_handler_extract_osm_pbf_data(
    data_path,
    threads,
    &inputs,
    true,
    true,
    true,
    true,
    None,
    None,
    false,
    None,
  );

  println!();
  println!("\x1b[2m── {}\x1b[0m", stage::extract_osm_admin_levels.name());
  let touched = command_handler_extract_osm_admin_levels(
    data_path,
    crate::presets::DEFAULT.extract_osm_admin_levels.admin_levels,
    threads,
    false,
    None,
  );

  println!();
  println!("\x1b[2m── {}\x1b[0m", stage::extract_osm_house_numbers.name());
  let numbered = command_handler_extract_osm_house_numbers(data_path, false);

  let isos: HashSet<String> = touched
    .iter()
    .map(|iso| iso.to_ascii_lowercase())
    .chain(numbered.iter().map(|iso| iso.to_ascii_lowercase()))
    .collect();
  let bases: Vec<base_files> = store::bases(data_path)
    .into_iter()
    .filter(|base| isos.contains(&base.iso))
    .collect();

  println!();
  println!("\x1b[2m── {}\x1b[0m", stage::index_admin_levels_hierarchy.name());
  let bases = index_bases(&bases);

  println!();
  println!("\x1b[2m── {}\x1b[0m", stage::optimize_delete_intermediary_data.name());
  command_handler_optimize(data_path, &bases);

  println!();
  store::seal_bases(&bases);
}
