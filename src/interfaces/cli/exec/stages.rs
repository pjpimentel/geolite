use rusqlite::Connection;

use crate::interfaces::cli::extract::resolved_input;

#[derive(Clone, Copy, PartialEq)]
pub enum stage {
  extract_osm_pbf_blob_chunks,
  extract_osm_pbf_header,
  extract_osm_pbf_data,
  extract_osm_admin_levels,
  extract_osm_house_numbers,
  index_admin_levels_hierarchy,
  optimize_merge_admin_levels,
  index_user_friendly_name,
  index_coordinates,
  optimize_delete_intermediary_data,
  optimize_sqlite_file,
}

impl stage {
  pub const fn name(self) -> &'static str {
    match self {
      stage::extract_osm_pbf_blob_chunks => "extract-osm-pbf-blob-chunks",
      stage::extract_osm_pbf_header => "extract-osm-pbf-header",
      stage::extract_osm_pbf_data => "extract-osm-pbf-data",
      stage::extract_osm_admin_levels => "extract-osm-admin-levels",
      stage::extract_osm_house_numbers => "extract-osm-house-numbers",
      stage::index_admin_levels_hierarchy => "index-admin-levels-hierarchy",
      stage::optimize_merge_admin_levels => "optimize-merge-admin-levels",
      stage::index_user_friendly_name => "index-user-friendly-name",
      stage::index_coordinates => "index-coordinates",
      stage::optimize_delete_intermediary_data => "optimize-delete-intermediary-data",
      stage::optimize_sqlite_file => "optimize-sqlite-file",
    }
  }

  pub fn requires(self) -> &'static [stage] {
    match self {
      stage::extract_osm_pbf_blob_chunks => &[],
      stage::extract_osm_pbf_header => &[stage::extract_osm_pbf_blob_chunks],
      stage::extract_osm_pbf_data => &[stage::extract_osm_pbf_blob_chunks],
      stage::extract_osm_admin_levels => &[stage::extract_osm_pbf_data],
      stage::extract_osm_house_numbers => &[stage::extract_osm_admin_levels],
      stage::index_admin_levels_hierarchy => &[stage::extract_osm_admin_levels],
      stage::optimize_merge_admin_levels => &[stage::index_admin_levels_hierarchy],
      stage::index_user_friendly_name => &[stage::index_admin_levels_hierarchy],
      stage::index_coordinates => &[stage::extract_osm_admin_levels],
      stage::optimize_delete_intermediary_data => &[
        stage::extract_osm_house_numbers,
        stage::index_admin_levels_hierarchy,
      ],
      stage::optimize_sqlite_file => &[],
    }
  }

  fn ran(self, conn: &Connection, file: Option<&resolved_input>) -> bool {
    match self {
      stage::extract_osm_pbf_blob_chunks => {
        file.is_some_and(|f| crate::osm_pbf_file::blob_index::count_by_file_id(conn, f.id) > 0)
      }
      stage::extract_osm_pbf_data => crate::osm_pbf_file::repository::any_osm_data_extracted(conn),
      stage::extract_osm_admin_levels => crate::admin_level::repository::count_with_geometry(conn) > 0,
      stage::extract_osm_house_numbers => {
        crate::house_number::repository::any(conn)
          || crate::osm_pbf_file::repository::any_house_numbers_counted(conn)
      }
      stage::index_admin_levels_hierarchy => {
        crate::admin_level_hierarchy::repository::count(conn) > 0
          && crate::admin_level_hierarchy::repository::pending_total(conn) == 0
      }
      other => unreachable!("{} is required by no stage", other.name()),
    }
  }

  pub(in crate::interfaces::cli) fn require(
    self,
    conn: &Connection,
    file: Option<&resolved_input>,
  ) {
    for needed in self.requires() {
      if !needed.ran(conn, file) {
        let input = file.map(|f| format!(" {}", f.name)).unwrap_or_default();
        eprintln!(
          "\x1b[1;31merror\x1b[0m: {} requires {} — run `geolite exec {}{input}` first",
          self.name(),
          needed.name(),
          needed.name()
        );
        std::process::exit(1);
      }
    }
  }
}
