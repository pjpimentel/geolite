pub mod build;
pub mod exec;
pub mod extract;
pub mod http_server;
pub mod index;
pub mod optimize;
pub mod osm_pbf_file;
mod progress;
pub mod query;
pub mod tui;

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
use exec::exec_commands;
use osm_pbf_file::osm_pbf_file_commands;

use crate::database::store::base_files;

pub(crate) fn progress_draw_target() -> indicatif::ProgressDrawTarget {
  indicatif::ProgressDrawTarget::stderr()
}

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

  #[command(subcommand)]
  command: commands,
}

#[derive(Subcommand)]
enum commands {
  #[command(name = "osm-pbf-file")]
  osm_pbf_file {
    #[command(subcommand)]
    command: osm_pbf_file_commands,

    #[arg(long, name = "ls-endpoint", help = "overrides the geofabrik index url")]
    ls_endpoint: Option<String>,
  },
  exec {
    #[command(subcommand)]
    command: exec_commands,
  },
  query {
    #[arg(allow_hyphen_values = true)]
    input: String,
    #[arg(long, value_parser = crate::address::validate_friendly_name_format)]
    friendly_name_format: Option<String>,
    #[arg(long, value_parser = crate::address::parse_min_quality)]
    min_quality: Option<f64>,
    #[arg(long, allow_hyphen_values = true, value_parser = crate::admin_level::geometry::parse_bounding_wkt)]
    bounding_wkt: Option<crate::admin_level::geometry::bounding_geometry>,
    #[arg(long, value_delimiter = ',', value_parser = crate::admin_level::level::parse)]
    last_admin_levels: Option<Vec<crate::admin_level::level>>,
    #[arg(long, action = clap::ArgAction::Set, default_value_t = true)]
    include_wkt: bool,
    #[arg(long, value_delimiter = ',')]
    countries: Option<Vec<String>>,
  },
  #[command(name = "http-server")]
  http_server {
    #[arg(long, default_value = "0.0.0.0")]
    host: String,
    #[arg(long, default_value_t = 8080)]
    port: u16,
  },
  build {
    #[arg(value_name = "id_or_url", num_args(1..))]
    sources: Vec<String>,
  },
  tui {
    #[arg(value_name = "path")]
    path: Option<String>,
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

fn preflight_checks(data_path: &str, sqlite_path: Option<&str>) {
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
  if let Some(sqlite_path) = sqlite_path {
    let sqlite = std::path::Path::new(sqlite_path);
    if sqlite.exists() && std::fs::File::open(sqlite).is_err() {
      known_errors::missing_read_permission.exit("no read access to file ".to_string() + sqlite_path);
    }
  }
}

// the bases a query, the server or the tui answer over: the one named by --sqlite-path, or every
// base of the data path
pub(crate) fn served_bases(data_path: &str, single: Option<&base_files>) -> Vec<base_files> {
  let bases = match single {
    Some(base) => vec![base.clone()],
    None => crate::database::store::bases(data_path),
  };
  if bases.is_empty() {
    eprintln!(
      "\x1b[1;31merror\x1b[0m: no base in {data_path} — first you need to build the data using geolite build"
    );
    std::process::exit(1);
  }
  for base in &bases {
    require_sqlite(&base.sqlite);
  }
  bases
}

pub(crate) fn select_bases(
  bases: &[base_files],
  countries: Option<&[String]>,
) -> Result<Vec<base_files>, String> {
  let Some(countries) = countries else {
    return Ok(bases.to_vec());
  };
  let mut selected: Vec<base_files> = Vec::with_capacity(countries.len());
  for iso in countries {
    let base = bases
      .iter()
      .find(|base| base.iso.eq_ignore_ascii_case(iso))
      .ok_or_else(|| format!("countries: no base for '{iso}'"))?;
    if !selected.iter().any(|picked| picked.iso == base.iso) {
      selected.push(base.clone());
    }
  }
  Ok(selected)
}

pub fn run() {
  let args = cli::parse();

  // TODO: implantar solucao para debug em release.
  if args.debug {
    crate::debug!("debug: on");
  }

  let single: Option<base_files> = args.sqlite_path.as_deref().map(|sqlite_path| {
    let mut base = base_files::of(sqlite_path);
    if let Some(index_path) = &args.index_path {
      base.index = index_path.clone();
    }
    base
  });

  preflight_checks(&args.data_path, single.as_ref().map(|base| base.sqlite.as_str()));

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
        &source,
        ls_endpoint.as_deref(),
        &recreate_cache,
      ),
      osm_pbf_file_commands::download { id_from_ls_or_url } => {
        osm_pbf_file::download::command_handler_osm_pbf_file_download(
          &args.data_path,
          &args.threads,
          &id_from_ls_or_url,
          ls_endpoint.as_deref(),
          args.abort_on_any_error,
        )
      }
    },
    commands::exec { command } => {
      exec::command_handler_exec(&args.data_path, &args.threads, command)
    }
    commands::query {
      input,
      friendly_name_format,
      min_quality,
      bounding_wkt,
      last_admin_levels,
      include_wkt,
      countries,
    } => query::command_handler_query(
      &args.data_path,
      single.as_ref(),
      countries.as_deref(),
      &input,
      crate::address::query_opts {
        friendly_name_format: friendly_name_format.as_deref(),
        min_quality,
        bounding: bounding_wkt,
        last_admin_levels,
        include_wkt,
      },
    ),
    commands::http_server { host, port } => http_server::command_handler_http_server(
      &args.data_path,
      single.as_ref(),
      crate::interfaces::http::bind(&format!("{host}:{port}")),
      args.threads,
    ),
    commands::build { sources } => build::command_handler_build(
      &args.data_path,
      &args.threads,
      &sources,
      None,
      args.abort_on_any_error,
    ),
    commands::tui { path } => {
      tui::command_handler_tui(&args.data_path, single.as_ref(), path.as_deref())
    }
  }
}
