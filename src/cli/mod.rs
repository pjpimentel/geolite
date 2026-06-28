pub mod build;
pub mod extract;
pub mod http_server;
pub mod index;
pub mod merge;
pub mod optimize;
pub mod osm_pbf_file;
pub mod query;

pub(crate) fn require_sqlite(path: &str) {
  if !std::path::Path::new(path).exists() {
    eprintln!("\x1b[1;31merror\x1b[0m: sqlite not found: {path}");
    std::process::exit(1);
  }
  if std::fs::File::open(path).is_err() {
    eprintln!("\x1b[1;31merror\x1b[0m: sqlite not readable: {path}");
    std::process::exit(1);
  }
}

use clap::{Parser, Subcommand};
use extract::extract_commands;
use index::index_commands;
use optimize::optimize_commands;
use osm_pbf_file::osm_pbf_file_commands;

const DEFAULT_GEOFABRIK_ENDPOINT: &str = "https://download.geofabrik.de/index-v1.json";

fn default_threads() -> u8 {
  std::thread::available_parallelism()
    .map(|n| ((n.get() - 1).max(1)) as u8)
    .unwrap_or(1)
}

fn default_data_dir() -> String {
  std::env::var("HOME")
    .map(|h| {
      std::path::Path::new(&h)
        .join(".geolite")
        .to_string_lossy()
        .into_owned()
    })
    .unwrap_or_else(|_| "./.geolite-data".to_string())
}

fn parse_min_quality(s: &str) -> Result<f64, String> {
  let v: f64 = s.parse().map_err(|e| format!("not a number: {e}"))?;
  if !(0.0..=1.0).contains(&v) {
    return Err(format!("must be between 0.0 and 1.0, got {v}"));
  }
  Ok(v)
}

#[derive(Parser)]
#[command(name = "geolite", version)]
struct cli {
  #[arg(long, default_value_t = false)]
  debug: bool,

  #[arg(long, default_value_t = default_data_dir())]
  data_path: String,

  #[arg(long)]
  sqlite_path: Option<String>,

  #[arg(long)]
  index_path: Option<String>,

  #[arg(long, default_value_t = default_threads())]
  threads: u8,

  #[arg(long, default_value_t = false)]
  abort_on_any_error: bool,

  #[arg(long)]
  preset: Option<String>,

  #[command(subcommand)]
  command: commands,
}

#[derive(Subcommand)]
enum commands {
  #[command(name = "osm-pbf-file")]
  osm_pbf_file {
    #[command(subcommand)]
    command: osm_pbf_file_commands,

    #[arg(long, name = "ls-endpoint", default_value = DEFAULT_GEOFABRIK_ENDPOINT)]
    ls_endpoint: String,
  },
  #[command(override_usage = "geolite extract [OPTIONS] <COMMAND> [OSM_PBF_FILE_PATH]")]
  extract {
    #[arg(long, default_value_t = false)]
    recreate: bool,
    #[command(subcommand)]
    command: extract_commands,
  },
  index {
    #[command(subcommand)]
    command: Option<index_commands>,
  },
  optimize {
    #[command(subcommand)]
    command: Option<optimize_commands>,
  },
  query {
    #[arg(allow_hyphen_values = true)]
    input: String,
    #[arg(long, value_parser = crate::query::validate_friendly_name_format)]
    friendly_name_format: Option<String>,
    #[arg(long, value_parser = parse_min_quality)]
    min_quality: Option<f64>,
    #[arg(long, allow_hyphen_values = true, value_parser = crate::http::parse_bounding_wkt)]
    bounding_wkt: Option<crate::query::bounding_geometry>,
    #[arg(long, value_delimiter = ',')]
    last_admin_levels: Option<Vec<u8>>,
    #[arg(long, action = clap::ArgAction::Set, default_value_t = true)]
    include_wkt: bool,
  },
  #[command(name = "http-server")]
  http_server {
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
    #[arg(long, default_value_t = 8080)]
    port: u16,
  },
  build {
    #[arg(value_name = "id_or_url")]
    source: String,
  },
  merge {
    #[arg(value_name = "base")]
    base: String,
    #[arg(value_name = "databases", num_args = 1..)]
    databases: Vec<String>,
  },
}

enum known_errors {
  missing_read_permission = 1,
  missing_write_permission = 2,
  // ops_unexpected_error_sorry = 99,
}
impl known_errors {
  fn exit(self, msg: impl std::fmt::Display) -> ! {
    let error_code = self as i32;
    eprintln!("error_code: {error_code}; {msg}");
    std::process::exit(error_code)
  }
}

fn preflight_checks(data_path: &str, sqlite_path: &str) {
  let data_dir = std::path::Path::new(data_path);
  if !data_dir.exists() && std::fs::create_dir_all(data_dir).is_err() {
    known_errors::missing_write_permission.exit("failed to create dir ".to_string() + data_path);
  }
  let probe = data_dir.join(".geolite_preflight");
  match std::fs::write(&probe, b"") {
    Err(_) => {
      known_errors::missing_write_permission
        .exit("no write access to dir ".to_string() + data_path);
    }
    Ok(_) => {
      let _ = std::fs::remove_file(&probe);
    }
  }
  if std::fs::read_dir(data_dir).is_err() {
    known_errors::missing_read_permission.exit("no read access to dir ".to_string() + data_path);
  }
  let sqlite = std::path::Path::new(sqlite_path);
  if sqlite.exists() && std::fs::File::open(sqlite).is_err() {
    known_errors::missing_read_permission.exit("no read access to file ".to_string() + sqlite_path);
  }
}

pub fn run() {
  let args = cli::parse();

  let preset = crate::presets::resolve(args.preset.clone());

  // TODO: implantar solucao para debug em release.
  if args.debug {
    crate::debug!("debug: on");
  }

  let sqlite_path: String = args
    .sqlite_path
    .unwrap_or_else(|| format!("{}/database.sqlite3", args.data_path));

  let explicit_index_path = args.index_path.clone();
  let index_path: String = args
    .index_path
    .or_else(|| {
      crate::index::admin_levels_hierarchy_tantivy::default_path_for(&sqlite_path)
        .map(|p| p.to_string_lossy().into_owned())
    })
    .unwrap_or_else(|| format!("{}/database.tantivy", args.data_path));

  preflight_checks(&args.data_path, &sqlite_path);

  match args.command {
    commands::osm_pbf_file {
      command,
      ls_endpoint,
    } => match command {
      osm_pbf_file_commands::ls {
        source,
        recreate_cache,
      } => osm_pbf_file::ls::command_handler_osm_pbf_file_ls(
        &args.data_path,
        &sqlite_path,
        &source,
        &ls_endpoint,
        &recreate_cache,
      ),
      osm_pbf_file_commands::download { id_from_ls_or_url } => {
        osm_pbf_file::download::command_handler_osm_pbf_file_download(
          &args.data_path,
          &args.threads,
          &sqlite_path,
          &id_from_ls_or_url,
          &ls_endpoint,
          args.abort_on_any_error,
        )
      }
    },
    commands::extract { command, recreate } => extract::command_handler_extract(
      &args.data_path,
      &args.threads,
      &sqlite_path,
      command,
      &recreate,
      &preset,
    ),
    commands::index { command } => {
      index::command_handler_index(
        &sqlite_path,
        &index_path,
        command,
        &preset.index_user_friendly_name,
      )
    }
    commands::optimize { command } => {
      optimize::command_handler_optimize(&args.data_path, &sqlite_path, &index_path, command)
    }
    commands::query {
      input,
      friendly_name_format,
      min_quality,
      bounding_wkt,
      last_admin_levels,
      include_wkt,
    } => query::command_handler_query(
      &sqlite_path,
      &index_path,
      &input,
      friendly_name_format.as_deref(),
      min_quality,
      bounding_wkt,
      last_admin_levels,
      include_wkt,
      preset.index_user_friendly_name.boosts,
    ),
    commands::http_server { host, port } => http_server::command_handler_http_server(
      &sqlite_path,
      &index_path,
      &host,
      port,
      args.threads,
      preset.index_user_friendly_name.boosts,
    ),
    commands::build { source } => {
      let preset = crate::presets::resolve(args.preset.or(Some(source.clone())));
      build::command_handler_build(
        &args.data_path,
        &args.threads,
        &sqlite_path,
        &index_path,
        &source,
        DEFAULT_GEOFABRIK_ENDPOINT,
        args.abort_on_any_error,
        &preset,
      )
    }
    commands::merge { base, databases } => {
      // the tantivy index belongs to the base (positional), not the global --sqlite-path default.
      let merge_index_path = explicit_index_path
        .or_else(|| {
          crate::index::admin_levels_hierarchy_tantivy::default_path_for(&base)
            .map(|p| p.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| format!("{}/database.tantivy", args.data_path));
      merge::command_handler_merge(&base, &databases, &merge_index_path, &preset)
    }
  }
}
