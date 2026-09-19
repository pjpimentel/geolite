#![allow(nonstandard_style)]
#[rustfmt::skip] mod address;
#[rustfmt::skip] mod admin_level;
#[rustfmt::skip] mod admin_level_hierarchy;
#[rustfmt::skip] mod house_number;
#[rustfmt::skip] mod osm_node;
#[rustfmt::skip] mod osm_pbf_file;
#[rustfmt::skip] mod osm_relation;
#[rustfmt::skip] mod osm_tag;
#[rustfmt::skip] mod osm_way;
#[rustfmt::skip] mod database; // 1
#[rustfmt::skip] mod interfaces;
#[rustfmt::skip] mod presets; // 7

#[macro_export]
macro_rules! debug {
  ($($arg:tt)*) => {
    if cfg!(debug_assertions) {
      eprintln!($($arg)*);
    }
  };
}

fn main() {
  interfaces::cli::run();
}
