use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;
use std::time::Instant;

fn level_name(level: u8) -> &'static str {
  match level {
    1 => "continent",
    2 => "country",
    3 => "region",
    4 => "state",
    5 => "district",
    6 => "county",
    7 => "municipality",
    8 => "city",
    9 => "locality",
    10 => "neighborhood",
    12 => "street",
    14 => "address",
    _ => "unknown",
  }
}

fn print_stage_header(label: &str) {
  println!("\x1b[2m{label}\x1b[0m");
}

fn extract_stage(
  label: &str,
  run: impl FnOnce(Box<dyn Fn(crate::extract::admin_levels::progress_report)>),
) -> f64 {
  let total = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
  let total_cb = total.clone();

  let bar = ProgressBar::new_spinner();
  bar.set_style(
    ProgressStyle::with_template(
      "{prefix:.bold.green} {msg:<16}  [{bar:20.green/white}] {percent:>3}%  {pos:>6}/{len:<6}  {per_sec}  eta {eta}",
    )
    .unwrap()
    .progress_chars("=> "),
  );
  bar.set_prefix("extracting");
  bar.set_message(label.to_string());

  let start = Instant::now();
  let bar_cb = bar.clone();

  run(Box::new(move |p| {
    if let Some(t) = p.total {
      if bar_cb.length().is_none() {
        bar_cb.set_length(t);
      }
      total_cb.store(t, std::sync::atomic::Ordering::Relaxed);
    }
    bar_cb.set_position(p.processed);
  }));

  bar.finish();
  let elapsed = start.elapsed().as_secs_f64();
  let remaining = total.load(std::sync::atomic::Ordering::Relaxed);

  if remaining == 0 {
    println!("\x1b[1;32mskipping\x1b[0m {label} — nothing to extract");
    return 0.0;
  }

  println!("\x1b[1;32mextracted\x1b[0m {remaining} {label} in {elapsed:.1}s");
  elapsed
}

pub fn command_handler_extract_osm_admin_levels(
  sqlite_path: &str,
  admin_levels: &[u8],
  threads: &u8,
  recreate: bool,
  name_priority: &[&str],
  rules: &[crate::extract::admin_levels::extraction_rules],
) {
  use crate::extract::admin_levels as ext;
  let threads = (*threads).max(1) as usize;

  if recreate {
    crate::database::destroy_data(sqlite_path, false, false, true, true);
  }

  let conn = crate::database::open_write(sqlite_path);
  let overrides: &[ext::extraction_rules] = rules;

  for (i, &level) in admin_levels.iter().enumerate() {
    if i > 0 {
      println!();
    }

    let name = level_name(level);
    println!("\x1b[1;32mlevel\x1b[0m {level} ({name})");

    match level {
      10 => {
        print_stage_header("stage 1/2: relations");

        print!("\x1b[1;32midentifying\x1b[0m candidates...");
        let _ = std::io::stdout().flush();
        let all_ids = crate::database::osm_relations::all_ids_by_admin_level(&conn, level);
        let remaining_ids =
          crate::database::osm_relations::remaining_ids_by_admin_level(&conn, level);
        println!(" done ({} found)", all_ids.len());

        print!("\x1b[1;32mremoving\x1b[0m already processed...");
        let _ = std::io::stdout().flush();
        println!(" done ({} remaining)", remaining_ids.len());

        extract_stage("neighborhood", |on_progress| {
          ext::run_with_ids(
            &conn,
            remaining_ids,
            ext::osm_admin_level::neighborhood,
            threads,
            name_priority,
            on_progress,
          );
        });

        println!();
        print_stage_header("stage 2/2: ways (place=neighbourhood,suburb)");

        extract_stage("neighborhood ways", |on_progress| {
          ext::level_10::run(&conn, overrides, name_priority, on_progress);
        });
      }

      12 => {
        extract_stage("street", |on_progress| {
          ext::level_12::run(&conn, overrides, name_priority, on_progress);
        });
      }

      _ => {
        print!("\x1b[1;32midentifying\x1b[0m candidates...");
        let _ = std::io::stdout().flush();
        let all_ids = crate::database::osm_relations::all_ids_by_admin_level(&conn, level);
        let remaining_ids =
          crate::database::osm_relations::remaining_ids_by_admin_level(&conn, level);
        println!(" done ({} found)", all_ids.len());

        print!("\x1b[1;32mremoving\x1b[0m already processed...");
        let _ = std::io::stdout().flush();
        println!(" done ({} remaining)", remaining_ids.len());

        let level_enum = match ext::osm_admin_level::try_from(level) {
          Ok(l) => l,
          Err(_) => {
            eprintln!("\x1b[1;31merror\x1b[0m: level {level} not supported");
            continue;
          }
        };
        extract_stage(name, |on_progress| {
          ext::run_with_ids(
            &conn,
            remaining_ids,
            level_enum,
            threads,
            name_priority,
            on_progress,
          );
        });
      }
    }
  }

  crate::database::admin_levels::create_indexes(&conn);

  crate::database::osm_pbf_files::update_admin_levels_count(&conn);
}
