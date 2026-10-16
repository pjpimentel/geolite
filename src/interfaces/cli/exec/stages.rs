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
  optimize_delete_isolated_admin_levels,
  optimize_merge_admin_levels,
  index_addresses,
  index_coordinates,
  optimize_delete_intermediary_data,
  optimize_sqlite_file,
}

// what a stage asserts against: the catalogue with the shared osm_data attached, the file when the
// stage has a source, and the bases it reads or writes
pub(in crate::interfaces::cli) struct stage_context<'a> {
  pub catalog: Option<&'a Connection>,
  pub file: Option<&'a resolved_input>,
  pub bases: Vec<&'a Connection>,
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
      stage::optimize_delete_isolated_admin_levels => "optimize-delete-isolated-admin-levels",
      stage::optimize_merge_admin_levels => "optimize-merge-admin-levels",
      stage::index_addresses => "index-addresses",
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
      stage::optimize_delete_isolated_admin_levels => &[stage::index_admin_levels_hierarchy],
      stage::optimize_merge_admin_levels => &[stage::index_admin_levels_hierarchy],
      stage::index_addresses => &[stage::index_admin_levels_hierarchy],
      stage::index_coordinates => &[stage::extract_osm_admin_levels],
      stage::optimize_delete_intermediary_data => &[
        stage::extract_osm_house_numbers,
        stage::index_admin_levels_hierarchy,
      ],
      stage::optimize_sqlite_file => &[],
    }
  }

  fn ran(self, ctx: &stage_context) -> bool {
    match self {
      stage::extract_osm_pbf_blob_chunks => match (ctx.catalog, ctx.file) {
        (Some(conn), Some(f)) => crate::osm_pbf_file::blob_index::count_by_file_id(conn, f.id) > 0,
        _ => false,
      },
      stage::extract_osm_pbf_data => match (ctx.catalog, ctx.file) {
        (Some(conn), Some(f)) => crate::osm_pbf_file::repository::osm_data_extracted(conn, f.id),
        (Some(conn), None) => crate::osm_pbf_file::repository::any_osm_data_extracted(conn),
        (None, _) => false,
      },
      stage::extract_osm_admin_levels => ctx
        .bases
        .iter()
        .any(|base| crate::admin_level::repository::count_with_geometry(base) > 0),
      stage::extract_osm_house_numbers => {
        ctx
          .bases
          .iter()
          .any(|base| crate::house_number::repository::any(base))
          || ctx
            .catalog
            .is_some_and(crate::osm_pbf_file::repository::any_house_numbers_counted)
      }
      stage::index_admin_levels_hierarchy => {
        !ctx.bases.is_empty()
          && ctx.bases.iter().all(|base| {
            crate::admin_level_hierarchy::repository::count(base) > 0
              && crate::admin_level_hierarchy::repository::pending_total(base) == 0
          })
      }
      other => unreachable!("{} is required by no stage", other.name()),
    }
  }

  pub(in crate::interfaces::cli) fn require(self, ctx: &stage_context) {
    for needed in self.requires() {
      if !needed.ran(ctx) {
        let input = ctx.file.map(|f| format!(" {}", f.name)).unwrap_or_default();
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
