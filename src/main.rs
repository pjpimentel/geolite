#![allow(nonstandard_style)]
#[rustfmt::skip] mod domain;
#[rustfmt::skip] mod cli; // 0
#[rustfmt::skip] mod database; // 1
#[rustfmt::skip] mod query; // 5
#[rustfmt::skip] mod http; // 6
#[rustfmt::skip] mod presets; // 7

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
  domain::osm_pbf_file::repository::get_file_path(&conn, input)
}

fn main() {
  cli::run();
}
