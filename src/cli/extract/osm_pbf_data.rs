use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::io::Write;
use std::time::Instant;

fn resolve_buffer_bytes(buffer_limit_in_mb: Option<u64>) -> usize {
  const MIB: u64 = 1024 * 1024;
  let mb = buffer_limit_in_mb.unwrap_or_else(|| {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let total_bytes = sys.total_memory();
    let target_bytes = total_bytes * 60 / 100;
    let mb = target_bytes / MIB;
    println!(
      "\x1b[1;36mbuffer-limit-in-mb\x1b[0m default = {mb} MiB (60% of {} MiB total ram)",
      total_bytes / MIB
    );
    mb
  });
  (mb * MIB) as usize
}

fn fmt_count(n: usize) -> String {
  if n >= 1_000_000 {
    format!("{:.1}M", n as f64 / 1_000_000.0)
  } else if n >= 1_000 {
    format!("{:.1}K", n as f64 / 1_000.0)
  } else {
    n.to_string()
  }
}

fn fmt_bytes(n: usize) -> String {
  const KIB: f64 = 1024.0;
  const MIB: f64 = KIB * 1024.0;
  const GIB: f64 = MIB * 1024.0;
  let f = n as f64;
  if f >= GIB {
    format!("{:.2}GiB", f / GIB)
  } else if f >= MIB {
    format!("{:.1}MiB", f / MIB)
  } else if f >= KIB {
    format!("{:.0}KiB", f / KIB)
  } else {
    format!("{n}B")
  }
}

#[allow(clippy::too_many_arguments)]
pub fn command_handler_extract_osm_pbf_data(
  data_path: &str,
  sqlite_path: &str,
  threads: &u8,
  inputs: &[String],
  include_relations: bool,
  include_ways: bool,
  include_nodes: bool,
  ignore_info: bool,
  tags_include_list: Option<String>,
  tags_ignore_list: Option<String>,
  recreate: bool,
  buffer_limit_in_mb: Option<u64>,
) {
  let buffer_bytes = resolve_buffer_bytes(buffer_limit_in_mb);
  if recreate {
    crate::database::destroy_data(sqlite_path, false, true, true, true);
  }

  let conn = crate::database::open_write(sqlite_path);

  for (i, input) in inputs.iter().enumerate() {
    if i > 0 {
      println!();
    }

    let is_path = std::path::Path::new(input.as_str()).exists()
      || std::path::Path::new(data_path)
        .join(input.as_str())
        .exists();

    if !is_path {
      print!("\x1b[1;32mresolving\x1b[0m '{input}'...");
      let _ = std::io::stdout().flush();
    }

    let resolved = crate::resolve_osm_pbf_path(data_path, sqlite_path, input);

    let osm_pbf_file_path = match resolved {
      Some(p) => {
        if !is_path {
          println!(" done");
        }
        p
      }
      None => {
        if !is_path {
          println!();
        }
        eprintln!("\x1b[1;31merror\x1b[0m: could not resolve '{input}'");
        continue;
      }
    };

    let fname = std::path::Path::new(&osm_pbf_file_path)
      .file_name()
      .unwrap_or_default()
      .to_string_lossy()
      .into_owned();

    println!("\x1b[1;32mfile\x1b[0m {osm_pbf_file_path}");

    let file_id = crate::database::osm_pbf_files::ensure_by_file_path(&conn, &osm_pbf_file_path);
    print!("\x1b[1;32mloading\x1b[0m blob-chunks index...");
    let _ = std::io::stdout().flush();
    let chunk_count = crate::database::osm_pbf_blob_chunks::count_by_file_id(&conn, file_id);

    if chunk_count == 0 {
      println!();
      eprintln!(
        "\x1b[1;31merror\x1b[0m: no blob chunks found — run extract osm-pbf-blob-chunks first"
      );
      continue;
    }

    println!(" done ({chunk_count} chunks)");

    let chunks = crate::database::osm_pbf_blob_chunks::get_data_chunks(&conn, file_id);

    let decoder_threads = threads.saturating_sub(1).max(1);
    let multi = MultiProgress::new();
    let decoder_bar = multi.add(ProgressBar::new_spinner());
    decoder_bar.set_style(
      ProgressStyle::with_template(
        "{prefix:.bold.green}  {msg:<80}  [{bar:20.green/white}] {percent:>3}%  {pos:>6}/{len:<6} chunks  {per_sec}  eta {eta}",
      )
      .unwrap()
      .progress_chars("=> "),
    );
    decoder_bar.set_prefix(format!("decode {decoder_threads:>2}t"));
    decoder_bar.set_message(fname.clone());
    decoder_bar.enable_steady_tick(std::time::Duration::from_millis(100));

    let writer_bar = multi.add(ProgressBar::new(0));
    writer_bar.set_style(
      ProgressStyle::with_template(
        "{prefix:.bold.cyan}  {msg:<80}  [{bar:20.cyan/blue}] {percent:>3}%  {human_pos:>6}/{human_len:<6} rows  {per_sec}  eta {eta}",
      )
      .unwrap()
      .progress_chars("=> "),
    );
    writer_bar.set_prefix(format!("writer {:>2}t", 1));
    writer_bar.set_message("nodes: 0  ways: 0  rel: 0  flushes: 0  bytes: 0B");
    writer_bar.enable_steady_tick(std::time::Duration::from_millis(100));

    let start = Instant::now();

    let tags_include = tags_include_list
      .as_deref()
      .map(|s| s.split(',').map(|t| t.trim().to_string()).collect());
    let tags_ignore = tags_ignore_list
      .as_deref()
      .map(|s| s.split(',').map(|t| t.trim().to_string()).collect());

    let decoder_bar_cb = decoder_bar.clone();
    let writer_bar_cb = writer_bar.clone();
    let write_conn = crate::database::open_write(sqlite_path);
    let (node_count, way_count, relation_count) = crate::extract::osm_data::run(
      &osm_pbf_file_path,
      chunks,
      write_conn,
      crate::extract::osm_data::data_opts {
        include_nodes,
        include_ways,
        include_relations,
        ignore_info,
        tags_include,
        tags_ignore,
        buffer_bytes,
      },
      threads,
      move |p| {
        if decoder_bar_cb.length().is_none() {
          decoder_bar_cb.set_length(p.total_chunks as u64);
        }
        if !decoder_bar_cb.is_finished() {
          decoder_bar_cb.set_position(p.chunks_done as u64);
          decoder_bar_cb.set_message(format!(
            "nodes: {:>8}  ways: {:>7}  rel: {:>6}",
            fmt_count(p.node_count),
            fmt_count(p.way_count),
            fmt_count(p.relation_count),
          ));
          if p.chunks_done >= p.total_chunks {
            decoder_bar_cb.finish();
          }
        }
        let rows_decoded = p.node_count + p.way_count + p.relation_count;
        let rows_written = p.nodes_written + p.ways_written + p.relations_written;
        writer_bar_cb.set_length(rows_decoded as u64);
        writer_bar_cb.set_position(rows_written as u64);
        writer_bar_cb.set_message(format!(
          "nodes: {:>8}  ways: {:>7}  rel: {:>6}  flushes: {:>4}  bytes: {:>9}",
          fmt_count(p.nodes_written),
          fmt_count(p.ways_written),
          fmt_count(p.relations_written),
          p.flushes_done,
          fmt_bytes(p.bytes_flushed),
        ));
      },
    );

    decoder_bar.finish();
    writer_bar.finish();

    crate::database::osm_pbf_files::update_counts(
      &conn,
      file_id,
      node_count as u64,
      way_count as u64,
      relation_count as u64,
    );

    let elapsed = start.elapsed().as_secs_f64();
    println!(
      "\x1b[1;32mextracted\x1b[0m {fname}  nodes: {}  ways: {}  relations: {}  in {elapsed:.1}s  ×{threads} threads",
      fmt_count(node_count),
      fmt_count(way_count),
      fmt_count(relation_count),
    );
  }

  let bar = ProgressBar::new_spinner();
  bar.set_style(ProgressStyle::with_template("{prefix:.bold.green} {msg} {spinner}").unwrap());
  bar.set_prefix("indexing");

  bar.set_message("osm_nodes");
  bar.enable_steady_tick(std::time::Duration::from_millis(100));
  let start = Instant::now();
  crate::database::osm_nodes::create_indexes(&conn);
  bar.disable_steady_tick();
  println!(
    "\x1b[1;32mindexed\x1b[0m osm_nodes in {:.1}s",
    start.elapsed().as_secs_f64()
  );

  bar.set_message("osm_ways");
  bar.enable_steady_tick(std::time::Duration::from_millis(100));
  let start = Instant::now();
  crate::database::osm_ways::create_indexes(&conn);
  bar.disable_steady_tick();
  println!(
    "\x1b[1;32mindexed\x1b[0m osm_ways in {:.1}s",
    start.elapsed().as_secs_f64()
  );

  bar.set_message("osm_relations");
  bar.enable_steady_tick(std::time::Duration::from_millis(100));
  let start = Instant::now();
  crate::database::osm_relations::create_indexes(&conn);
  bar.finish_and_clear();
  println!(
    "\x1b[1;32mindexed\x1b[0m osm_relations in {:.1}s",
    start.elapsed().as_secs_f64()
  );
}
