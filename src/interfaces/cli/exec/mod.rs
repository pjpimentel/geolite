pub mod stages;

use clap::Subcommand;

use super::extract::{
  osm_admin_levels, osm_house_numbers, osm_pbf_blob_chunks, osm_pbf_data, osm_pbf_header,
};
use super::index::{addresses, admin_levels_hierarchy, coordinates};
use super::optimize::{
  delete_intermediary_data, delete_isolated_admin_levels, merge_admin_levels, sqlite_file,
};
use crate::admin_level::level;
use crate::database::store::{self, base_files};
use stages::stage;

#[derive(Subcommand)]
pub enum exec_commands {
  #[command(name = stage::extract_osm_pbf_blob_chunks.name())]
  extract_osm_pbf_blob_chunks {
    #[arg(num_args(1..))]
    inputs: Vec<String>,
    #[arg(long, default_value_t = false)]
    recreate: bool,
  },
  #[command(name = stage::extract_osm_pbf_header.name())]
  extract_osm_pbf_header {
    #[arg(num_args(1..))]
    inputs: Vec<String>,
  },
  #[command(name = stage::extract_osm_pbf_data.name())]
  extract_osm_pbf_data {
    #[arg(num_args(1..))]
    inputs: Vec<String>,

    #[arg(long, action = clap::ArgAction::Set, default_value_t = true)]
    include_relations: bool,

    #[arg(long, action = clap::ArgAction::Set, default_value_t = true)]
    include_ways: bool,

    #[arg(long, action = clap::ArgAction::Set, default_value_t = true)]
    include_nodes: bool,

    #[arg(long, default_value_t = true)]
    ignore_info: bool,

    #[arg(long)]
    tags_include_list: Option<String>,

    #[arg(long)]
    tags_ignore_list: Option<String>,

    #[arg(long, default_value_t = false)]
    recreate: bool,

    // hard limit of the writer buffer in MiB; the flush fires at 80%.
    // default: 60% of the ram visible to the process.
    #[arg(long, value_name = "MB")]
    buffer_limit_in_mb: Option<u64>,
  },
  #[command(name = stage::extract_osm_admin_levels.name())]
  extract_osm_admin_levels {
    #[arg(long)]
    admin_level: Option<String>,

    #[arg(long, default_value_t = false)]
    recreate: bool,

    // priority list of osm tags used to populate admin_levels.name.
    // tags are tried in order; first non-null wins.
    // examples:
    //   --name-priority "name"                  (native osm name)
    //   --name-priority "name:pt,name:en,name"  (prefer pt, fallback en, then default)
    // when omitted, the priority of the country of each element decides.
    #[arg(long, value_name = "TAGS")]
    name_priority: Option<String>,
  },
  #[command(name = stage::extract_osm_house_numbers.name())]
  extract_osm_house_numbers {
    #[arg(long, default_value_t = false)]
    recreate: bool,
  },
  #[command(name = stage::index_admin_levels_hierarchy.name())]
  index_admin_levels_hierarchy {
    #[arg(value_name = "country", num_args(0..))]
    countries: Vec<String>,
  },
  #[command(name = stage::optimize_delete_isolated_admin_levels.name())]
  optimize_delete_isolated_admin_levels {
    #[arg(value_name = "country", num_args(0..))]
    countries: Vec<String>,
  },
  #[command(name = stage::optimize_merge_admin_levels.name())]
  optimize_merge_admin_levels {
    #[arg(value_name = "country", num_args(0..))]
    countries: Vec<String>,
  },
  #[command(name = stage::index_addresses.name())]
  index_addresses {
    #[arg(value_name = "country", num_args(0..))]
    countries: Vec<String>,
  },
  #[command(name = stage::index_coordinates.name())]
  index_coordinates {
    #[arg(value_name = "country", num_args(0..))]
    countries: Vec<String>,
  },
  #[command(name = stage::optimize_delete_intermediary_data.name())]
  optimize_delete_intermediary_data,
  #[command(name = stage::optimize_sqlite_file.name())]
  optimize_sqlite_file {
    #[arg(value_name = "country", num_args(0..))]
    countries: Vec<String>,
  },
}

// the bases a stage runs over: the ones named, or every base of the data path
fn bases_for(data_path: &str, countries: &[String]) -> Vec<base_files> {
  let all = store::bases(data_path);
  if countries.is_empty() {
    if all.is_empty() {
      eprintln!(
        "\x1b[1;31merror\x1b[0m: no base in {data_path} — first you need to build the data using geolite build"
      );
      std::process::exit(1);
    }
    return all;
  }
  countries
    .iter()
    .map(|iso| {
      all
        .iter()
        .find(|base| base.iso.eq_ignore_ascii_case(iso))
        .cloned()
        .unwrap_or_else(|| {
          eprintln!("\x1b[1;31merror\x1b[0m: no base for {iso} in {data_path}");
          std::process::exit(1);
        })
    })
    .collect()
}

// the bases a write stage runs over: a sealed base named is refused, a sealed base met is left as
// it is
fn writable_bases_for(data_path: &str, countries: &[String], stage: stage) -> Vec<base_files> {
  let bases = bases_for(data_path, countries);
  if !countries.is_empty()
    && let Some(base) = bases.iter().find(|base| base.is_sealed())
  {
    eprintln!(
      "\x1b[1;31merror\x1b[0m: {} is sealed — a base built once is final; remove it to run {} again",
      base.label(),
      stage.name()
    );
    std::process::exit(1);
  }
  let (sealed, writable): (Vec<base_files>, Vec<base_files>) =
    bases.into_iter().partition(|base| base.is_sealed());
  for base in &sealed {
    println!("\x1b[1;33msealed\x1b[0m {} — left as it is", base.label());
  }
  if writable.is_empty() {
    eprintln!("\x1b[1;31merror\x1b[0m: every base in {data_path} is sealed — a base built once is final");
    std::process::exit(1);
  }
  writable
}

fn per_base(bases: &[base_files], stage: impl Fn(&base_files)) {
  for (i, base) in bases.iter().enumerate() {
    if i > 0 {
      println!();
    }
    println!("\x1b[2mbase {}\x1b[0m", base.label());
    stage(base);
  }
}

fn with_stale_indexes(
  data_path: &str,
  bases: &[base_files],
  handler: fn(&str, &str) -> bool,
) {
  super::optimize::with_sizes(data_path, || {
    per_base(bases, |base| {
      if handler(&base.sqlite, &base.index) {
        println!(
          "\x1b[1;33mnext\x1b[0m run `geolite exec {} {}` and `geolite exec {} {}`",
          stage::index_addresses.name(),
          base.iso,
          stage::index_coordinates.name(),
          base.iso
        );
      }
    })
  })
}

pub fn command_handler_exec(data_path: &str, threads: &u8, command: exec_commands) {
  match command {
    exec_commands::extract_osm_pbf_blob_chunks { inputs, recreate } => {
      osm_pbf_blob_chunks::command_handler_extract_osm_pbf_blob_chunks(data_path, &inputs, recreate);
    }
    exec_commands::extract_osm_pbf_header { inputs } => {
      osm_pbf_header::command_handler_extract_osm_pbf_header(data_path, &inputs);
    }
    exec_commands::extract_osm_pbf_data {
      inputs,
      include_relations,
      include_ways,
      include_nodes,
      ignore_info,
      tags_include_list,
      tags_ignore_list,
      recreate,
      buffer_limit_in_mb,
    } => {
      osm_pbf_data::command_handler_extract_osm_pbf_data(
        data_path,
        threads,
        &inputs,
        include_relations,
        include_ways,
        include_nodes,
        ignore_info,
        tags_include_list,
        tags_ignore_list,
        recreate,
        buffer_limit_in_mb,
      );
    }
    exec_commands::extract_osm_admin_levels {
      admin_level,
      recreate,
      name_priority,
    } => {
      let levels: Vec<level> = match admin_level.as_deref() {
        Some(raw) => match super::extract::parse_admin_levels(raw) {
          Ok(v) => v,
          Err(e) => {
            eprintln!("\x1b[1;31merror\x1b[0m: invalid --admin-level: {e}");
            std::process::exit(1);
          }
        },
        None => crate::presets::DEFAULT.extract_osm_admin_levels.admin_levels.to_vec(),
      };
      let names: Option<Vec<&str>> = match name_priority.as_deref() {
        Some(raw) => match super::extract::parse_name_priority(raw) {
          Ok(v) => Some(v),
          Err(e) => {
            eprintln!("\x1b[1;31merror\x1b[0m: invalid --name-priority: {e}");
            std::process::exit(1);
          }
        },
        None => None,
      };
      osm_admin_levels::command_handler_extract_osm_admin_levels(
        data_path,
        &levels,
        threads,
        recreate,
        names.as_deref(),
      );
    }
    exec_commands::extract_osm_house_numbers { recreate } => {
      osm_house_numbers::command_handler_extract_osm_house_numbers(data_path, recreate);
    }
    exec_commands::index_admin_levels_hierarchy { countries } => {
      per_base(&writable_bases_for(data_path, &countries, stage::index_admin_levels_hierarchy), |base| {
        admin_levels_hierarchy::command_handler_index_admin_levels_hierarchy(&base.sqlite)
      })
    }
    exec_commands::index_addresses { countries } => {
      per_base(&writable_bases_for(data_path, &countries, stage::index_addresses), |base| {
        addresses::command_handler_index_addresses(&base.sqlite, &base.index)
      })
    }
    exec_commands::index_coordinates { countries } => {
      per_base(&writable_bases_for(data_path, &countries, stage::index_coordinates), |base| {
        coordinates::command_handler_index_coordinates(&base.sqlite)
      })
    }
    exec_commands::optimize_delete_isolated_admin_levels { countries } => with_stale_indexes(
      data_path,
      &writable_bases_for(data_path, &countries, stage::optimize_delete_isolated_admin_levels),
      delete_isolated_admin_levels::command_handler_optimize_delete_isolated_admin_levels,
    ),
    exec_commands::optimize_merge_admin_levels { countries } => with_stale_indexes(
      data_path,
      &writable_bases_for(data_path, &countries, stage::optimize_merge_admin_levels),
      merge_admin_levels::command_handler_optimize_merge_admin_levels,
    ),
    exec_commands::optimize_delete_intermediary_data => {
      super::optimize::with_sizes(data_path, || {
        delete_intermediary_data::command_handler_optimize_delete_intermediary_data(data_path);
      })
    }
    exec_commands::optimize_sqlite_file { countries } => {
      let bases = writable_bases_for(data_path, &countries, stage::optimize_sqlite_file);
      super::optimize::with_sizes(data_path, || {
        per_base(&bases, |base| {
          sqlite_file::command_handler_optimize_sqlite_file(&base.sqlite)
        })
      })
    }
  }
}
