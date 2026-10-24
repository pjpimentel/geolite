use std::path::Path;

use crate::address::{base, query_input, query_opts};
use crate::admin_level_hierarchy::{tantivy_boosts, tantivy_index};
use crate::database::store::base_files;

pub fn command_handler_query(
  data_path: &str,
  single: Option<&base_files>,
  countries: Option<&[String]>,
  text: &str,
  opts: query_opts,
) {
  let served = crate::interfaces::cli::served_bases(data_path, single);
  let mut selected = match crate::interfaces::cli::select_bases(&served, countries) {
    Ok(selected) => selected,
    Err(message) => {
      eprintln!("\x1b[1;31merror\x1b[0m: {message}");
      std::process::exit(1);
    }
  };
  let input = query_input::parse(text);
  if let query_input::coordinates {
    latitude,
    longitude,
  } = input
  {
    let country = crate::address::country_of_point(latitude, longitude);
    selected.retain(|base| country.is_some_and(|iso| base.iso.eq_ignore_ascii_case(iso)));
  }
  let boosts = crate::presets::DEFAULT.index_addresses.boosts;
  let mut opened = open_at_once(&selected, boosts);
  if input == query_input::text && opened.iter().all(|(_, index)| index.is_none()) {
    eprintln!(
      "\x1b[1;31merror\x1b[0m: tantivy index not found for {} — run `geolite exec index-addresses` first",
      selected
        .iter()
        .map(|base| base.label())
        .collect::<Vec<_>>()
        .join(", ")
    );
    std::process::exit(1);
  }
  let labels: Vec<String> = selected.iter().map(base_files::label).collect();
  let mut bases: Vec<base> = labels
    .iter()
    .zip(&selected)
    .zip(&mut opened)
    .map(|((file, files), (conn, index))| base {
      file,
      iso: &files.iso,
      conn,
      index: index.as_ref(),
    })
    .collect();
  let result = match input {
    query_input::coordinates {
      latitude,
      longitude,
    } => crate::address::query_by_coordinates_across(&mut bases, latitude, longitude, &opts),
    query_input::text => crate::address::query_by_text_across(&mut bases, text, &opts),
  };
  println!("{}", serde_json::to_string_pretty(&result).unwrap());
}

fn open_at_once(
  bases: &[base_files],
  boosts: tantivy_boosts,
) -> Vec<(rusqlite::Connection, Option<tantivy_index>)> {
  crate::address::in_parallel(bases.iter().collect(), |files| {
    (
      crate::database::store::open_base_immutable(&files.sqlite),
      crate::admin_level_hierarchy::search_index::load(Path::new(&files.index), boosts),
    )
  })
}
