use std::io::Write;

pub fn command_handler_extract_osm_pbf_header(
  data_path: &str,
  sqlite_path: &str,
  inputs: &[String],
) {
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

    print!("\x1b[1;32mextracting\x1b[0m header from {fname}...");
    let _ = std::io::stdout().flush();

    let hdr = crate::extract::header::run(&osm_pbf_file_path, &conn, file_id);

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
