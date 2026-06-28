pub mod download;
pub mod ls;

use clap::{Subcommand, ValueEnum};

#[derive(Subcommand)]
pub enum osm_pbf_file_commands {
  ls {
    #[arg(value_enum, default_value_t = osm_pbf_file_ls_source::geofabrik)]
    source: osm_pbf_file_ls_source,

    #[arg(long, default_value_t = false)]
    recreate_cache: bool,
  },
  download {
    #[arg(value_name = "URL_OR_ID", num_args = 1..)]
    id_from_ls_or_url: Vec<String>,
  },
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum osm_pbf_file_ls_source {
  geofabrik,
  local,
}
