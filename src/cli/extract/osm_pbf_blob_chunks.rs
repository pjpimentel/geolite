use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;
use std::time::Instant;

pub fn command_handler_extract_osm_pbf_blob_chunks(
  data_path: &str,
  sqlite_path: &str,
  inputs: &[String],
  recreate: bool,
) {
  if recreate {
    crate::database::destroy_data(sqlite_path, true, true, true, true);
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

    let file_id = crate::database::osm_pbf_files::ensure_by_file_path(&conn, &osm_pbf_file_path);

    let bar = ProgressBar::new_spinner();
    bar.set_style(
      ProgressStyle::with_template(
        "{prefix:.bold.green} {msg:<40}  [{bar:20.green/white}] {percent:>3}%  {bytes:>10} / {total_bytes:<10}  {binary_bytes_per_sec}",
      )
      .unwrap()
      .progress_chars("=> "),
    );
    bar.set_prefix("extracting");
    bar.set_message(fname.clone());

    let start = Instant::now();

    let count = crate::extract::blob_chunks::run(&osm_pbf_file_path, &conn, file_id, |p| {
      if bar.length().is_none() {
        bar.set_length(p.total_bytes);
      }
      bar.set_position(p.bytes_read);
    });

    bar.finish();

    let elapsed = start.elapsed().as_secs_f64();
    println!("\x1b[1;32mextracted\x1b[0m {count} chunks from {fname} in {elapsed:.1}s");
  }

  crate::database::osm_pbf_blob_chunks::create_indexes(&conn);
}
