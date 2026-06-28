use std::path::Path;

use crate::index::admin_levels_hierarchy_tantivy::tantivy_index;

pub struct progress_report {
  pub total: Option<i64>,
  pub processed: i64,
}

pub fn run(
  conn: &rusqlite::Connection,
  index_path: &Path,
  preset: &crate::presets::index_user_friendly_name_preset,
  progress: impl Fn(progress_report),
) -> tantivy_index {
  let total = crate::database::admin_levels_hierarchy::count(conn);
  progress(progress_report {
    total: Some(total),
    processed: 0,
  });
  let index = crate::index::admin_levels_hierarchy_tantivy::build(
    conn,
    index_path,
    preset.boosts,
    preset.abbreviations,
  );
  progress(progress_report {
    total: Some(total),
    processed: total,
  });
  index
}
