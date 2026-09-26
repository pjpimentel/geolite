pub mod stages;

use clap::Subcommand;

use super::extract::{
  osm_admin_levels, osm_house_numbers, osm_pbf_blob_chunks, osm_pbf_data, osm_pbf_header,
};
use super::index::{admin_levels_hierarchy, coordinates, user_friendly_name};
use super::optimize::{delete_intermediary_data, merge_admin_levels, sqlite_file};
use crate::admin_level::level;
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
    // when omitted, falls back to the active preset's name_priority.
    #[arg(long, value_name = "TAGS")]
    name_priority: Option<String>,
  },
  #[command(name = stage::extract_osm_house_numbers.name())]
  extract_osm_house_numbers {
    #[arg(long, default_value_t = false)]
    recreate: bool,
  },
  #[command(name = stage::index_admin_levels_hierarchy.name())]
  index_admin_levels_hierarchy,
  #[command(name = stage::optimize_merge_admin_levels.name())]
  optimize_merge_admin_levels,
  #[command(name = stage::index_user_friendly_name.name())]
  index_user_friendly_name,
  #[command(name = stage::index_coordinates.name())]
  index_coordinates,
  #[command(name = stage::optimize_delete_intermediary_data.name())]
  optimize_delete_intermediary_data,
  #[command(name = stage::optimize_sqlite_file.name())]
  optimize_sqlite_file,
}

pub fn command_handler_exec(
  data_path: &str,
  threads: &u8,
  sqlite_path: &str,
  index_path: &str,
  command: exec_commands,
  preset: &crate::presets::preset,
) {
  match command {
    exec_commands::extract_osm_pbf_blob_chunks { inputs, recreate } => {
      osm_pbf_blob_chunks::command_handler_extract_osm_pbf_blob_chunks(
        data_path,
        sqlite_path,
        &inputs,
        recreate,
      );
    }
    exec_commands::extract_osm_pbf_header { inputs } => {
      osm_pbf_header::command_handler_extract_osm_pbf_header(data_path, sqlite_path, &inputs);
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
        sqlite_path,
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
        None => preset.extract_osm_admin_levels.admin_levels.to_vec(),
      };
      let names: Vec<&str> = match name_priority.as_deref() {
        Some(raw) => match super::extract::parse_name_priority(raw) {
          Ok(v) => v,
          Err(e) => {
            eprintln!("\x1b[1;31merror\x1b[0m: invalid --name-priority: {e}");
            std::process::exit(1);
          }
        },
        None => preset.extract_osm_admin_levels.name_priority.to_vec(),
      };
      osm_admin_levels::command_handler_extract_osm_admin_levels(
        sqlite_path,
        &levels,
        threads,
        recreate,
        &names,
        preset.extract_osm_admin_levels.admin_levels_rules,
      );
    }
    exec_commands::extract_osm_house_numbers { recreate } => {
      osm_house_numbers::command_handler_extract_osm_house_numbers(
        sqlite_path,
        recreate,
        preset.house_numbers,
      );
    }
    exec_commands::index_admin_levels_hierarchy => {
      admin_levels_hierarchy::command_handler_index_admin_levels_hierarchy(sqlite_path)
    }
    exec_commands::index_user_friendly_name => {
      user_friendly_name::command_handler_index_user_friendly_name(
        sqlite_path,
        index_path,
        &preset.index_user_friendly_name,
      )
    }
    exec_commands::index_coordinates => coordinates::command_handler_index_coordinates(sqlite_path),
    exec_commands::optimize_merge_admin_levels => {
      super::optimize::with_sqlite_sizes(sqlite_path, index_path, || {
        if merge_admin_levels::command_handler_optimize_merge_admin_levels(sqlite_path, index_path) {
          println!(
            "\x1b[1;33mnext\x1b[0m run `geolite exec {}` and `geolite exec {}`",
            stage::index_user_friendly_name.name(),
            stage::index_coordinates.name()
          );
        }
      })
    }
    exec_commands::optimize_delete_intermediary_data => {
      super::optimize::with_sqlite_sizes(sqlite_path, index_path, || {
        delete_intermediary_data::command_handler_optimize_delete_intermediary_data(
          data_path,
          sqlite_path,
        );
      })
    }
    exec_commands::optimize_sqlite_file => {
      super::optimize::with_sqlite_sizes(sqlite_path, index_path, || {
        sqlite_file::command_handler_optimize_sqlite_file(sqlite_path)
      })
    }
  }
}
