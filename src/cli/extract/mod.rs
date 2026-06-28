pub mod osm_admin_levels;
pub mod osm_house_numbers;
pub mod osm_pbf_blob_chunks;
pub mod osm_pbf_data;
pub mod osm_pbf_header;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum extract_commands {
  #[command(name = "osm-pbf-blob-chunks")]
  osm_pbf_blob_chunks {
    #[arg(num_args(1..))]
    inputs: Vec<String>,
    #[arg(long, default_value_t = false)]
    recreate: bool,
  },
  #[command(name = "osm-pbf-header")]
  osm_pbf_header {
    #[arg(num_args(1..))]
    inputs: Vec<String>,
  },
  #[command(name = "osm-pbf-data")]
  osm_pbf_data {
    #[arg(num_args(1..))]
    inputs: Vec<String>,

    #[arg(long, default_value_t = true)]
    include_relations: bool,

    #[arg(long, default_value_t = true)]
    include_ways: bool,

    #[arg(long, default_value_t = true)]
    include_nodes: bool,

    #[arg(long, default_value_t = true)]
    ignore_info: bool,

    #[arg(long)]
    tags_include_list: Option<String>,

    #[arg(long)]
    tags_ignore_list: Option<String>,

    #[arg(long, default_value_t = false)]
    recreate: bool,

    // hard limit do buffer do writer em MiB; flush dispara aos 80%.
    // default: 60% da ram total visivel ao processo.
    #[arg(long, value_name = "MB")]
    buffer_limit_in_mb: Option<u64>,
  },
  #[command(name = "osm-admin-levels")]
  osm_admin_levels {
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
  #[command(name = "osm-house-numbers")]
  osm_house_numbers {
    #[arg(long, default_value_t = false)]
    recreate: bool,
  },
}

// valid tag chars: ascii letters, digits, `:`, `-`, `_`. covers `name`,
// `name:en`, `name:pt-BR`, `int_name`, etc. rejects whitespace and quotes
// which would break the dynamic SQL JSON path.
fn parse_name_priority(raw: &str) -> Result<Vec<&str>, String> {
  let tags: Vec<&str> = raw
    .split(',')
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .collect();
  if tags.is_empty() {
    return Err("at least one tag required".to_string());
  }
  for tag in &tags {
    if !tag
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '-' || c == '_')
    {
      return Err(format!("invalid characters in tag '{tag}'"));
    }
  }
  Ok(tags)
}

pub fn command_handler_extract(
  data_path: &str,
  threads: &u8,
  sqlite_path: &str,
  command: extract_commands,
  _recreate: &bool,
  preset: &crate::presets::preset,
) {
  match command {
    // None => {
    //   let path = osm_pbf_file_path.unwrap_or_else(|| {
    //     eprintln!("error: osm_pbf_file_path is required when no subcommand is given");
    //     std::process::exit(1);
    //   });
    //   osm_pbf_blob_chunks::command_handler_extract_osm_pbf_blob_chunks(sqlite_path, path, *recreate);
    //   osm_pbf_header::command_handler_extract_osm_pbf_header(sqlite_path, path);
    //   osm_pbf_data::command_handler_extract_osm_pbf_data(
    //     sqlite_path, threads, path, true, true, true, true, None, None, *recreate,
    //   );
    //   osm_admin_levels::command_handler_extract_osm_admin_levels(sqlite_path, "2,4,8,10,12", *recreate);
    //   osm_house_numbers::command_handler_extract_osm_house_numbers(sqlite_path, *recreate);
    // }
    extract_commands::osm_pbf_blob_chunks { inputs, recreate } => {
      osm_pbf_blob_chunks::command_handler_extract_osm_pbf_blob_chunks(
        data_path,
        sqlite_path,
        &inputs,
        recreate,
      );
    }
    extract_commands::osm_pbf_header { inputs } => {
      osm_pbf_header::command_handler_extract_osm_pbf_header(data_path, sqlite_path, &inputs);
    }
    extract_commands::osm_pbf_data {
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
    extract_commands::osm_admin_levels {
      admin_level,
      recreate,
      name_priority,
    } => {
      let levels: Vec<u8> = match admin_level.as_deref() {
        Some(raw) => raw
          .split(',')
          .filter_map(|s| s.trim().parse().ok())
          .collect(),
        None => preset.extract_osm_admin_levels.admin_levels.to_vec(),
      };
      let names: Vec<&str> = match name_priority.as_deref() {
        Some(raw) => match parse_name_priority(raw) {
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
    extract_commands::osm_house_numbers { recreate } => {
      osm_house_numbers::command_handler_extract_osm_house_numbers(
        sqlite_path,
        recreate,
        preset.extract_house_numbers,
      );
    }
  }
}
