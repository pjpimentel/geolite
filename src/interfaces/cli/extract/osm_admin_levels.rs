use indicatif::ProgressBar;
use std::time::Instant;

use crate::admin_level::extract::{source, stage};
use crate::admin_level::{
  admin_level, extract_event, extract_opts, extract_step, extraction_rules, level,
};
use crate::database::table;
use crate::interfaces::cli::progress;
use crate::progress_report;

struct running_stage {
  label: String,
  bar: Option<ProgressBar>,
  started_at: Option<Instant>,
  total: u64,
}

#[derive(Default)]
struct stage_renderer {
  current: Option<running_stage>,
}

impl stage_renderer {
  fn on(&mut self, event: extract_event) {
    match event.step {
      extract_step::started => self.start(event.stage),
      extract_step::candidates { found, pending } => {
        println!("\x1b[1;32midentifying\x1b[0m candidates... done ({found} found)");
        println!("\x1b[1;32mremoving\x1b[0m already processed... done ({pending} remaining)");
      }
      extract_step::progress(report) => self.advance(report),
      extract_step::finished => self.finish(),
    }
  }

  fn start(&mut self, stage: stage) {
    if stage.of > 1 {
      if stage.ordinal > 1 {
        println!();
      }
      println!(
        "\x1b[2mstage {}/{}: {}\x1b[0m",
        stage.ordinal,
        stage.of,
        header_of(stage.source)
      );
    }
    self.current = Some(running_stage {
      label: label_of(stage),
      bar: None,
      started_at: None,
      total: 0,
    });
  }

  fn advance(&mut self, report: progress_report) {
    let Some(running) = &mut self.current else {
      return;
    };
    if running.bar.is_none() {
      running.started_at = Some(Instant::now());
      running.bar = Some(progress::bar("extracting", &running.label));
    }
    if let Some(total) = report.total {
      running.total = total;
    }
    progress::advance(running.bar.as_ref().unwrap(), &report);
  }

  fn finish(&mut self) {
    let Some(running) = self.current.take() else {
      return;
    };
    if let Some(bar) = &running.bar {
      bar.finish();
    }
    if running.total == 0 {
      println!(
        "\x1b[1;32mskipping\x1b[0m {} — nothing to extract",
        running.label
      );
      return;
    }
    let elapsed = running
      .started_at
      .map_or(0.0, |started_at| started_at.elapsed().as_secs_f64());
    println!(
      "\x1b[1;32mextracted\x1b[0m {} {} in {elapsed:.1}s",
      running.total, running.label
    );
  }
}

fn header_of(source: source) -> &'static str {
  match source {
    source::relations => "relations",
    source::place_ways => "ways (place=neighbourhood,suburb)",
    source::streets => "streets",
  }
}

fn label_of(stage: stage) -> String {
  match stage.source {
    source::relations => stage.level.name().to_string(),
    source::place_ways => "neighborhood ways".to_string(),
    source::streets => "street".to_string(),
  }
}

pub fn command_handler_extract_osm_admin_levels(
  sqlite_path: &str,
  admin_levels: &[level],
  threads: &u8,
  recreate: bool,
  name_priority: &[&str],
  rules: &[extraction_rules],
) {
  if recreate {
    crate::database::destroy_data(sqlite_path, false, false, true, true);
  }

  let conn = crate::database::open_write(sqlite_path);
  let opts = extract_opts {
    threads: (*threads).max(1) as usize,
    name_priority,
    rules,
  };
  let mut renderer = stage_renderer::default();

  for (i, &level) in admin_levels.iter().enumerate() {
    if i > 0 {
      println!();
    }

    println!(
      "\x1b[1;32mlevel\x1b[0m {} ({})",
      level.value(),
      level.name()
    );

    admin_level::extract(&conn, level, &opts, |event| renderer.on(event));
  }

  crate::admin_level::admin_levels::create_indexes(&conn);

  crate::osm_pbf_file::repository::update_admin_levels_count(&conn);
}
