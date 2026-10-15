use indicatif::ProgressBar;
use std::time::Instant;

use crate::admin_level::country_index::country_index;
use crate::admin_level::extract::{source, stage};
use crate::admin_level::{admin_level, extract_event, extract_opts, extract_step, level};
use crate::database::store::{self, base_pool};
use crate::database::table;
use crate::interfaces::cli::exec::stages::{stage as exec_stage, stage_context};
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
      extract_step::finished { kept, dropped } => self.finish(kept, dropped),
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

  fn finish(&mut self, kept: u64, dropped: u64) {
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
      "\x1b[1;32mextracted\x1b[0m {kept} {} in {elapsed:.1}s",
      running.label
    );
    if dropped > 0 {
      println!(
        "\x1b[1;33mdropped\x1b[0m {dropped} {} — the base of their country is sealed",
        running.label
      );
    }
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
  data_path: &str,
  source: &str,
  admin_levels: &[level],
  threads: &u8,
  recreate: bool,
  name_priority: Option<&[&str]>,
) -> Vec<&'static str> {
  let catalog = store::open_catalog(data_path);
  let Some(resolved) = super::resolve_input(&catalog, data_path, 0, source) else {
    std::process::exit(1);
  };
  let conn = store::open_source(data_path, &resolved.stem);
  let mut pool = base_pool::new(data_path);
  for base in pool.on_disk() {
    if base.is_sealed() {
      println!("\x1b[1;33msealed\x1b[0m {} — takes no row", base.file_name());
    }
  }
  if recreate {
    for base in pool.writable_on_disk() {
      store::destroy_base(&base.sqlite, true, true);
    }
  }
  exec_stage::extract_osm_admin_levels.require(&stage_context {
    catalog: Some(&conn),
    file: Some(&resolved),
    bases: vec![],
  });
  let mut name_tags: Vec<&str> = crate::presets::name_tags();
  for &tag in name_priority.unwrap_or(&[]) {
    if !name_tags.contains(&tag) {
      name_tags.push(tag);
    }
  }
  let mut renderer = stage_renderer::default();
  let countries = country_index::from_boundaries();

  for (i, &level) in admin_levels.iter().enumerate() {
    if i > 0 {
      println!();
    }

    println!(
      "\x1b[1;32mlevel\x1b[0m {} ({})",
      level.value(),
      level.name()
    );

    let opts = extract_opts {
      threads: (*threads).max(1) as usize,
      name_priority,
      name_tags: &name_tags,
      countries: &countries,
    };
    admin_level::extract(&conn, level, &opts, &mut pool, |event| renderer.on(event));
  }

  for (_, base) in pool.open_bases() {
    crate::admin_level::admin_levels::create_indexes(base);
  }
  let touched: Vec<&'static str> = pool.touched().to_vec();
  let bases: Vec<String> = pool.on_disk().into_iter().map(|base| base.sqlite).collect();
  drop(pool);
  crate::osm_pbf_file::osm_pbf_file::open(Some(&conn), data_path)
    .stamp_admin_levels_count(resolved.id, &bases);
  println!();
  println!("\x1b[1;32mbases\x1b[0m {}", bases_label(&touched));
  touched
}

pub(crate) fn bases_label(isos: &[&str]) -> String {
  if isos.is_empty() {
    return "none".to_string();
  }
  isos
    .iter()
    .map(|iso| format!("{}.sqlite3", iso.to_ascii_lowercase()))
    .collect::<Vec<_>>()
    .join(", ")
}
