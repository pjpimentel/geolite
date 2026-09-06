use super::resolved_input;
use std::io::Write;

pub fn command_handler_extract_osm_pbf_header(
  data_path: &str,
  sqlite_path: &str,
  inputs: &[String],
) {
  let conn = crate::database::open_write(sqlite_path);

  for (i, input) in inputs.iter().enumerate() {
    let Some(resolved_input {
      path: osm_pbf_file_path,
      name: fname,
      id: file_id,
    }) = super::resolve_input(&conn, data_path, sqlite_path, i, input)
    else {
      continue;
    };

    print!("\x1b[1;32mextracting\x1b[0m header from {fname}...");
    let _ = std::io::stdout().flush();

    let hdr = crate::domain::osm_pbf_file::header::run(&osm_pbf_file_path, &conn, file_id);

    println!(" done");

    if let Some(bbox) = &hdr.bbox {
      println!(
        "\x1b[1;32mbbox\x1b[0m       {:.4},{:.4}  {:.4},{:.4}",
        bbox.left, bbox.bottom, bbox.right, bbox.top
      );
    }
    if let Some(p) = &hdr.writingprogram {
      println!("\x1b[1;32mprogram\x1b[0m    {p}");
    }
    if let Some(s) = &hdr.source {
      println!("\x1b[1;32msource\x1b[0m     {s}");
    }
    if let Some(ts) = hdr.replication_timestamp {
      println!("\x1b[1;32mreplicated\x1b[0m {ts}");
    }
  }
}

#[cfg(test)]
#[path = "osm_pbf_header.test.rs"]
mod tests;
