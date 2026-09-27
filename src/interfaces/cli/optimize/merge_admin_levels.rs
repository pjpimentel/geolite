use std::io::Write;
use std::path::Path;
use std::time::Instant;

use crate::admin_level_hierarchy::street_merge;
use crate::interfaces::cli::exec::stages::stage;
use crate::interfaces::cli::progress;

pub fn command_handler_optimize_merge_admin_levels(sqlite_path: &str, index_path: &str) -> bool {
  crate::interfaces::cli::require_sqlite(sqlite_path);
  let conn = crate::database::open_write(sqlite_path);
  stage::optimize_merge_admin_levels.require(&conn, None);
  let start = Instant::now();

  let bar = progress::bar("finding", "street pieces");
  let pieces = street_merge::find(&conn, |p| progress::advance(&bar, &p));
  bar.finish();
  if pieces.is_empty() {
    println!("\x1b[1;32mskipping\x1b[0m merge-admin-levels — no street to merge");
    return false;
  }

  print!("\x1b[1;32mclearing\x1b[0m user-friendly-name and coordinates...");
  let _ = std::io::stdout().flush();
  crate::admin_level_hierarchy::search_index::destroy(Path::new(index_path));
  crate::admin_level::spatial_index::recreate(&conn);
  println!(" done");

  let bar = progress::bar("merging", "streets");
  let report = street_merge::fold(&conn, &pieces, |p| progress::advance(&bar, &p));
  bar.finish();

  crate::osm_pbf_file::repository::update_admin_levels_count(&conn);
  crate::osm_pbf_file::repository::update_house_numbers_count(&conn);

  let elapsed = start.elapsed().as_secs_f64();
  println!(
    "\x1b[1;32mmerged\x1b[0m {} street ways into {} streets in {elapsed:.1}s, {} house numbers moved",
    report.absorbed + report.pieces,
    report.pieces,
    report.numbers_moved
  );
  true
}
