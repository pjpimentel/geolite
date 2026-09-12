use crate::cli::extract::{
  osm_admin_levels::command_handler_extract_osm_admin_levels,
  osm_house_numbers::command_handler_extract_osm_house_numbers,
  osm_pbf_blob_chunks::command_handler_extract_osm_pbf_blob_chunks,
  osm_pbf_data::command_handler_extract_osm_pbf_data,
  osm_pbf_header::command_handler_extract_osm_pbf_header,
};
use crate::cli::index::command_handler_index;
use crate::cli::optimize::command_handler_optimize;
use crate::cli::osm_pbf_file::download::command_handler_osm_pbf_file_download;
use crate::domain::osm_pbf_file::{input_kind, osm_pbf_file};

#[allow(clippy::too_many_arguments)]
pub fn command_handler_build(
  data_path: &str,
  threads: &u8,
  sqlite_path: &str,
  index_path: &str,
  source: &str,
  ls_endpoint: Option<&str>,
  abort_on_any_error: bool,
  preset: &crate::presets::preset,
) {
  println!("\x1b[2mpreset: {}\x1b[0m", preset.name);
  println!();
  // filesystem only, on purpose: the database may not exist yet at this point of the build, and a
  // local source must never reach resolve_geofabrik_url, which fetches the geofabrik index with an
  // `expect` that would take the whole pipeline down when the network fails
  let is_local = osm_pbf_file::open(None, data_path)
    .local_file(source)
    .is_some();
  if input_kind::of(source) == input_kind::local_path && !is_local {
    eprintln!("\x1b[1;31merror\x1b[0m: source file not found: {source}");
    std::process::exit(1);
  }

  let inputs = vec![source.to_string()];

  println!("\x1b[2m── download\x1b[0m");
  if is_local {
    println!("\x1b[1;33mskipping\x1b[0m download — '{source}' is already a local file");
  } else {
    command_handler_osm_pbf_file_download(
      data_path,
      threads,
      sqlite_path,
      &inputs,
      ls_endpoint,
      abort_on_any_error,
    );
  }

  let resolved = is_local || {
    let conn = crate::database::open_readonly(sqlite_path);
    osm_pbf_file::open(Some(&conn), data_path)
      .resolve(source)
      .is_some()
  };
  if !resolved {
    eprintln!(
      "\x1b[1;31merror\x1b[0m: could not resolve source after download — aborting pipeline"
    );
    std::process::exit(1);
  }

  println!();
  println!("\x1b[2m── extract blob-chunks\x1b[0m");
  command_handler_extract_osm_pbf_blob_chunks(data_path, sqlite_path, &inputs, false);

  println!();
  println!("\x1b[2m── extract header\x1b[0m");
  command_handler_extract_osm_pbf_header(data_path, sqlite_path, &inputs);

  println!();
  println!("\x1b[2m── extract osm-data\x1b[0m");
  command_handler_extract_osm_pbf_data(
    data_path,
    sqlite_path,
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
  println!("\x1b[2m── extract admin-levels\x1b[0m");
  command_handler_extract_osm_admin_levels(
    sqlite_path,
    preset.extract_osm_admin_levels.admin_levels,
    threads,
    false,
    preset.extract_osm_admin_levels.name_priority,
    preset.extract_osm_admin_levels.admin_levels_rules,
  );

  println!();
  println!("\x1b[2m── extract house-numbers\x1b[0m");
  command_handler_extract_osm_house_numbers(sqlite_path, false, preset.house_numbers);

  println!();
  println!("\x1b[2m── index\x1b[0m");
  command_handler_index(sqlite_path, index_path, None, &preset.index_user_friendly_name);

  println!();
  println!("\x1b[2m── optimize\x1b[0m");
  command_handler_optimize(data_path, sqlite_path, index_path, None);
}

#[cfg(test)]
#[path = "build.test.rs"]
mod tests;
