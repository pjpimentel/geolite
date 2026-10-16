use std::path::Path;

use crate::address::{base, query_input, query_opts};
use crate::admin_level_hierarchy::tantivy_index;
use crate::database::store::base_files;

pub fn command_handler_query(
  data_path: &str,
  single: Option<&base_files>,
  countries: Option<&[String]>,
  text: &str,
  opts: query_opts,
) {
  let served = crate::interfaces::cli::served_bases(data_path, single);
  let selected = match crate::interfaces::cli::select_bases(&served, countries) {
    Ok(selected) => selected,
    Err(message) => {
      eprintln!("\x1b[1;31merror\x1b[0m: {message}");
      std::process::exit(1);
    }
  };
  let boosts = crate::presets::DEFAULT.index_addresses.boosts;
  let conns: Vec<rusqlite::Connection> = selected
    .iter()
    .map(|base| crate::database::store::open_base_immutable(&base.sqlite))
    .collect();
  let indexes: Vec<Option<tantivy_index>> = selected
    .iter()
    .map(|base| crate::admin_level_hierarchy::search_index::load(Path::new(&base.index), boosts))
    .collect();
  let input = query_input::parse(text);
  if input == query_input::text && indexes.iter().all(Option::is_none) {
    eprintln!(
      "\x1b[1;31merror\x1b[0m: tantivy index not found for {} — run `geolite exec index-addresses` first",
      selected
        .iter()
        .map(|base| base.file_name())
        .collect::<Vec<_>>()
        .join(", ")
    );
    std::process::exit(1);
  }
  let bases: Vec<base> = selected
    .iter()
    .zip(&conns)
    .zip(&indexes)
    .map(|((_, conn), index)| base {
      conn,
      index: index.as_ref(),
    })
    .collect();
  let result = match input {
    query_input::coordinates {
      latitude,
      longitude,
    } => crate::address::query_by_coordinates_across(&bases, latitude, longitude, &opts),
    query_input::text => crate::address::query_by_text_across(&bases, text, &opts),
  };
  println!("{}", serde_json::to_string_pretty(&result).unwrap());
}
