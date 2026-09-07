use super::resolved_input;
use crate::domain::table;
use indicatif::{ProgressBar, ProgressStyle};
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
  let file = crate::domain::osm_pbf_file::osm_pbf_file::open(Some(&conn), data_path);

  for (i, input) in inputs.iter().enumerate() {
    let Some(resolved_input {
      path: osm_pbf_file_path,
      name: fname,
      id: _,
    }) = super::resolve_input(&conn, data_path, sqlite_path, i, input)
    else {
      continue;
    };

    let bar = ProgressBar::new_spinner();
    bar.set_draw_target(crate::cli::progress_draw_target());
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

    let count = file.extract_blob_chunks(&osm_pbf_file_path, |p| {
      if bar.length().is_none() {
        bar.set_length(p.total_bytes);
      }
      bar.set_position(p.bytes_read);
    });

    bar.finish();

    let elapsed = start.elapsed().as_secs_f64();
    println!("\x1b[1;32mextracted\x1b[0m {count} chunks from {fname} in {elapsed:.1}s");
  }

  crate::domain::osm_pbf_file::osm_pbf_blob_chunks::create_indexes(&conn);
}

#[cfg(test)]
#[path = "osm_pbf_blob_chunks.test.rs"]
mod tests;
