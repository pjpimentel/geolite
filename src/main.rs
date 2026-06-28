#![allow(nonstandard_style)]
#[rustfmt::skip] mod cli; // 0
#[rustfmt::skip] mod database; // 1
#[rustfmt::skip] mod osm_pbf_file; // 2
#[rustfmt::skip] mod extract; // 3
#[rustfmt::skip] mod index; // 4
#[rustfmt::skip] mod optimize; // 5
#[rustfmt::skip] mod query; // 6
#[rustfmt::skip] mod http; // 7
#[rustfmt::skip] mod presets; // 8

#[macro_export]
macro_rules! debug {
  ($($arg:tt)*) => {
    if cfg!(debug_assertions) {
      eprintln!($($arg)*);
    }
  };
}

fn resolve_osm_pbf_path(data: &str, sqlite_path: &str, input: &str) -> Option<String> {
  let p = std::path::Path::new(input);
  if p.exists() {
    return Some(input.to_string());
  }
  let in_data = std::path::Path::new(data).join(input);
  if in_data.exists() {
    return in_data.to_str().map(|s| s.to_string());
  }
  let conn = database::open_readonly(sqlite_path);
  database::osm_pbf_files::get_file_path(&conn, input)
}

fn main() {
  cli::run();
}
