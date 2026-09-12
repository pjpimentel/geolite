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

fn main() {
  cli::run();
}
