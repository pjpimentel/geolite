use crate::osm_pbf_file::download::{download_event, md5_status, run};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

pub fn command_handler_osm_pbf_file_download(
  data_path: &str,
  threads: &u8,
  sqlite_path: &str,
  inputs: &[String],
  ls_endpoint: &str,
  abort_on_any_error: bool,
) {
  for (i, input) in inputs.iter().enumerate() {
    if i > 0 {
      println!();
    }

    let is_url = input.starts_with("http://") || input.starts_with("https://");
    let url = if is_url {
      input.clone()
    } else {
      print!("\x1b[1;32mresolving\x1b[0m url for '{input}'...");
      let _ = std::io::stdout().flush();
      match crate::osm_pbf_file::ls::resolve_geofabrik_url(sqlite_path, input, ls_endpoint) {
        Some(u) => {
          println!(" done");
          u
        }
        None => {
          eprintln!(
            "\n\x1b[1;31merror\x1b[0m: '{input}' is not a valid url nor a known geofabrik id"
          );
          if abort_on_any_error {
            std::process::exit(1);
          }
          continue;
        }
      }
    };

    println!("\x1b[1;32mfetching\x1b[0m {url}");

    let pb: Arc<OnceLock<ProgressBar>> = Arc::new(OnceLock::new());
    let start_time: Arc<OnceLock<Instant>> = Arc::new(OnceLock::new());
    let fname = Arc::new(
      url
        .split('/')
        .next_back()
        .unwrap_or("file.osm.pbf")
        .to_string(),
    );

    let pb_cb = pb.clone();
    let start_cb = start_time.clone();
    let fname_cb = fname.clone();

    let output = run(data_path, &url, *threads, move |event| match event {
      download_event::download_start { total, .. } => {
        let _ = start_cb.set(Instant::now());
        let b = ProgressBar::new(total);
        b.set_style(
          ProgressStyle::with_template(
            "{prefix:.bold.green} {msg:<40}  [{bar:20.green/white}] {percent:>3}%  {bytes:>10} / {total_bytes:<10}  {binary_bytes_per_sec}",
          )
          .unwrap()
          .progress_chars("=> "),
        );
        b.set_prefix("downloading");
        b.set_message((*fname_cb).clone());
        let _ = pb_cb.set(b);
      }
      download_event::download_progress { delta } => {
        if let Some(bar) = pb_cb.get() {
          bar.inc(delta);
        }
      }
      download_event::merging => {
        if let Some(bar) = pb_cb.get() {
          bar.finish();
        }
        print!("\x1b[1;32mmerging\x1b[0m parts...");
        let _ = std::io::stdout().flush();
      }
      download_event::merge_progress { .. } => {}
      download_event::verifying_md5 => {
        println!(" done");
        print!("\x1b[1;32mverifying\x1b[0m md5...");
        let _ = std::io::stdout().flush();
      }
      download_event::file_already_exists { path } => {
        println!(
          "\x1b[1;33mwarning\x1b[0m: file already exists at {} — reusing",
          path.display()
        );
      }
    });

    match output {
      None => {
        if abort_on_any_error {
          std::process::exit(1);
        }
      }
      Some(output) => {
        let fname = output
          .path
          .file_name()
          .unwrap_or_default()
          .to_string_lossy();
        match start_time.get() {
          Some(t) => {
            let elapsed = t.elapsed().as_secs_f64();
            println!(" done");
            println!(
              "\x1b[1;32msaved\x1b[0m {fname} in {elapsed:.1}s → {}",
              output.path.display()
            );
          }
          None => {
            println!(
              "\x1b[1;32mreused\x1b[0m {fname} → {}",
              output.path.display()
            );
          }
        }
        if let md5_status::mismatch { expected, actual } = &output.md5 {
          eprintln!("\x1b[1;33mwarning\x1b[0m: md5 mismatch: expected {expected} got {actual}");
        }
        let conn = crate::database::open_write(sqlite_path);
        crate::database::osm_pbf_files::create_indexes(&conn);
        crate::database::osm_pbf_files::update_downloaded(
          &conn,
          &url,
          output.path.to_str().unwrap_or(""),
          output.total_bytes,
          &output.actual_md5,
        );
      }
    }
  }
}
