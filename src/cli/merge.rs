use crate::cli::index::command_handler_index;

fn require_compatible_version(label: &str, path: &str) {
  let version = crate::database::read_user_version(path);
  if version != crate::database::SCHEMA_VERSION {
    eprintln!(
      "\x1b[1;31merror\x1b[0m: incompatible schema version on {label} {path}: found {version}, expected {} — rebuild required",
      crate::database::SCHEMA_VERSION
    );
    std::process::exit(1);
  }
}

pub fn command_handler_merge(
  base: &str,
  databases: &[String],
  index_path: &str,
  preset: &crate::presets::preset,
) {
  if databases.is_empty() {
    eprintln!(
      "\x1b[1;31merror\x1b[0m: no databases to merge — usage: geolite merge <base> <db1> [db2 ...]"
    );
    std::process::exit(1);
  }
  for db in databases {
    if !std::path::Path::new(db).exists() {
      eprintln!("\x1b[1;31merror\x1b[0m: database not found: {db}");
      std::process::exit(1);
    }
  }

  // the base may be an existing build (merged into, in-place) or a path that does not exist yet
  // (created fresh and populated entirely from the sources). only an existing base is
  // version-checked — a fresh one is created at the current SCHEMA_VERSION by open_write_main.
  // validate BEFORE mutating the base, since open_write_main would otherwise re-stamp its
  // user_version and mask a mismatch.
  if std::path::Path::new(base).exists() {
    require_compatible_version("base", base);
  } else {
    println!("\x1b[2mcreating new base {base}\x1b[0m");
  }
  for db in databases {
    require_compatible_version("source", db);
  }

  // merge the raw extracted data (admin_levels + house_numbers), source by source.
  {
    let conn = crate::database::open_write_main(base);
    crate::database::admin_levels::drop_indexes(&conn);
    crate::database::house_numbers::drop_indexes(&conn);

    for db in databases {
      let (admins, houses) = crate::database::merge::merge_source(&conn, db);
      println!("\x1b[1;32mmerged\x1b[0m {db}  admin_levels: {admins}  house_numbers: {houses}");
    }

    if crate::database::admin_levels::count_with_geometry(&conn) == 0 {
      eprintln!("\x1b[1;31merror\x1b[0m: admin_levels is empty after merge — aborting");
      std::process::exit(1);
    }

    crate::database::admin_levels::create_indexes(&conn);
    crate::database::house_numbers::create_indexes(&conn);
  }

  // rebuild every derived artifact (hierarchy + rtree + tantivy) from the unified set; each index
  // step clears and rebuilds from scratch, so no stale rows survive the merge.
  println!();
  println!("\x1b[2m── index\x1b[0m");
  command_handler_index(base, index_path, None, &preset.index_user_friendly_name);

  // compact the resulting file (ANALYZE / PRAGMA optimize / wal_checkpoint / VACUUM).
  println!();
  println!("\x1b[2m── optimize\x1b[0m");
  let conn = crate::database::open_write_main(base);
  let (bytes_before, bytes_after) = crate::optimize::sqlite_file::run(&conn);
  println!("\x1b[1;32moptimized\x1b[0m {bytes_before} → {bytes_after} bytes");
}

#[cfg(test)]
#[path = "merge.test.rs"]
mod tests;
