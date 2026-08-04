use super::osm_pbf_file::osm_pbf_file_ls_source;
use super::*;
use clap::Parser;
use std::path::PathBuf;

fn unique_tag() -> String {
  static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
  let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
  format!("{}_{seq}", std::process::id())
}

// isolated temp dir; Drop re-opens permissions on direct children (some tests chmod them
// restrictive) before removing everything.
struct temp_base {
  base: PathBuf,
}

impl temp_base {
  fn new(tag: &str) -> Self {
    let base = std::env::temp_dir().join(format!("geolite_cli_test_{tag}_{}", unique_tag()));
    std::fs::create_dir_all(&base).expect("failed to create temp base dir");
    temp_base { base }
  }
}

impl Drop for temp_base {
  fn drop(&mut self) {
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      let _ = std::fs::set_permissions(&self.base, std::fs::Permissions::from_mode(0o755));
      if let Ok(entries) = std::fs::read_dir(&self.base) {
        for entry in entries.flatten() {
          let _ = std::fs::set_permissions(entry.path(), std::fs::Permissions::from_mode(0o755));
        }
      }
    }
    let _ = std::fs::remove_dir_all(&self.base);
  }
}

#[cfg(unix)]
fn chmod(path: &std::path::Path, mode: u32) {
  use std::os::unix::fs::PermissionsExt;
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
    .expect("failed to chmod test path");
}

fn parse(args: &[&str]) -> cli {
  cli::try_parse_from(args).expect("args should parse")
}

// re-runs this test binary filtered to a single #[ignore]d `_90_*` helper. this is how code
// paths that call process::exit are asserted: the child dies, the parent checks exit code and
// stderr. llvm-cov merges the child's profile automatically (inherited LLVM_PROFILE_FILE).
// pub(crate) so the other src/cli/*.test.rs modules reuse the same runner.
pub(crate) fn respawn(helper: &str, envs: &[(&str, &str)], env_remove: &[&str]) -> std::process::Output {
  let mut cmd = std::process::Command::new(std::env::current_exe().expect("current_exe"));
  cmd.args(["--exact", helper, "--ignored", "--nocapture"]);
  for (key, value) in envs {
    cmd.env(key, value);
  }
  for key in env_remove {
    cmd.env_remove(key);
  }
  cmd.output().expect("failed to respawn test binary")
}

pub(crate) fn stderr_of(out: &std::process::Output) -> String {
  String::from_utf8_lossy(&out.stderr).into_owned()
}

// a level-12 street row at a slight lon offset — the shared fixture for the index, optimize and
// query cli tests, which only need "some indexed streets on disk".
pub(crate) fn street_row(
  name: &str,
  way_id: u64,
  lon_offset: f64,
) -> crate::database::admin_levels::admin_levels {
  use geo::{Coord, Geometry, LineString};
  crate::database::admin_levels::admin_levels {
    relation_id: None,
    way_id: Some(way_id),
    admin_level: 12,
    wkb: Geometry::LineString(LineString(vec![
      Coord { x: -46.3198 + lon_offset, y: -23.9724 },
      Coord { x: -46.3197 + lon_offset, y: -23.9724 },
    ]))
    .into(),
    name: name.to_string(),
    country_iso_code: None,
    post_code: None,
  }
}

#[test]
fn _00_00_parse_min_quality_accepts_boundaries_and_midpoint() {
  assert_eq!(parse_min_quality("0.0"), Ok(0.0));
  assert_eq!(parse_min_quality("0.5"), Ok(0.5));
  assert_eq!(parse_min_quality("1.0"), Ok(1.0));
}

#[test]
fn _00_01_parse_min_quality_rejects_non_number() {
  let err = parse_min_quality("abc").expect_err("non-number must be rejected");
  assert!(err.contains("not a number"), "unexpected error: {err}");
}

#[test]
fn _00_02_parse_min_quality_rejects_values_outside_range() {
  for input in ["-0.1", "1.1"] {
    let err = parse_min_quality(input).expect_err("out-of-range must be rejected");
    assert!(err.contains("must be between 0.0 and 1.0"), "unexpected error: {err}");
  }
}

#[test]
fn _01_00_default_threads_is_at_least_one() {
  assert!(default_threads() >= 1);
}

#[test]
fn _01_01_default_data_dir_ends_with_geolite_when_home_is_set() {
  // HOME is always set on dev/ci machines; the unset fallback is asserted by _04_06.
  assert!(default_data_dir().ends_with(".geolite"));
}

#[test]
fn _02_00_defaults_populate_data_path_threads_and_flags() {
  let args = parse(&["geolite", "index"]);
  assert!(!args.debug);
  assert_eq!(args.data_path, default_data_dir());
  assert_eq!(args.sqlite_path, None);
  assert_eq!(args.index_path, None);
  assert_eq!(args.threads, default_threads());
  assert!(!args.abort_on_any_error);
  assert_eq!(args.preset, None);
  assert!(matches!(args.command, commands::index { command: None }));
}

#[test]
fn _02_01_global_flags_are_accepted_before_the_subcommand() {
  let args = parse(&[
    "geolite",
    "--debug",
    "--data-path",
    "/tmp/geolite-data",
    "--sqlite-path",
    "/tmp/db.sqlite3",
    "--index-path",
    "/tmp/db.tantivy",
    "--threads",
    "3",
    "--abort-on-any-error",
    "--preset",
    "brazil",
    "index",
  ]);
  assert!(args.debug);
  assert_eq!(args.data_path, "/tmp/geolite-data");
  assert_eq!(args.sqlite_path.as_deref(), Some("/tmp/db.sqlite3"));
  assert_eq!(args.index_path.as_deref(), Some("/tmp/db.tantivy"));
  assert_eq!(args.threads, 3);
  assert!(args.abort_on_any_error);
  assert_eq!(args.preset.as_deref(), Some("brazil"));
}

#[test]
fn _02_02_osm_pbf_file_ls_parses_sources_endpoint_and_recreate_cache() {
  let args = parse(&["geolite", "osm-pbf-file", "ls"]);
  match args.command {
    commands::osm_pbf_file { command, ls_endpoint } => {
      assert_eq!(ls_endpoint, DEFAULT_GEOFABRIK_ENDPOINT);
      match command {
        osm_pbf_file_commands::ls { source, recreate_cache } => {
          assert!(source == osm_pbf_file_ls_source::geofabrik);
          assert!(!recreate_cache);
        }
        _ => panic!("expected ls subcommand"),
      }
    }
    _ => panic!("expected osm-pbf-file command"),
  }

  let args = parse(&[
    "geolite",
    "osm-pbf-file",
    "--ls-endpoint",
    "http://127.0.0.1:1/index.json",
    "ls",
    "local",
    "--recreate-cache",
  ]);
  match args.command {
    commands::osm_pbf_file { command, ls_endpoint } => {
      assert_eq!(ls_endpoint, "http://127.0.0.1:1/index.json");
      match command {
        osm_pbf_file_commands::ls { source, recreate_cache } => {
          assert!(source == osm_pbf_file_ls_source::local);
          assert!(recreate_cache);
        }
        _ => panic!("expected ls subcommand"),
      }
    }
    _ => panic!("expected osm-pbf-file command"),
  }
}

#[test]
fn _02_03_osm_pbf_file_download_parses_multiple_inputs() {
  let args = parse(&["geolite", "osm-pbf-file", "download", "sul", "https://x/y.osm.pbf"]);
  match args.command {
    commands::osm_pbf_file { command, .. } => match command {
      osm_pbf_file_commands::download { id_from_ls_or_url } => {
        assert_eq!(id_from_ls_or_url, vec!["sul", "https://x/y.osm.pbf"]);
      }
      _ => panic!("expected download subcommand"),
    },
    _ => panic!("expected osm-pbf-file command"),
  }
}

fn parse_extract(args: &[&str]) -> (bool, extract_commands) {
  match parse(args).command {
    commands::extract { recreate, command } => (recreate, command),
    _ => panic!("expected extract command"),
  }
}

#[test]
fn _02_04_extract_parses_all_five_subcommands() {
  let (recreate, command) =
    parse_extract(&["geolite", "extract", "--recreate", "osm-pbf-blob-chunks", "a.pbf", "b.pbf", "--recreate"]);
  assert!(recreate);
  match command {
    extract_commands::osm_pbf_blob_chunks { inputs, recreate } => {
      assert_eq!(inputs, vec!["a.pbf", "b.pbf"]);
      assert!(recreate);
    }
    _ => panic!("expected osm-pbf-blob-chunks"),
  }

  let (recreate, command) = parse_extract(&["geolite", "extract", "osm-pbf-header", "a.pbf"]);
  assert!(!recreate);
  match command {
    extract_commands::osm_pbf_header { inputs } => assert_eq!(inputs, vec!["a.pbf"]),
    _ => panic!("expected osm-pbf-header"),
  }

  let (_, command) = parse_extract(&[
    "geolite",
    "extract",
    "osm-pbf-data",
    "a.pbf",
    "--tags-include-list",
    "name,highway",
    "--tags-ignore-list",
    "bogus",
    "--buffer-limit-in-mb",
    "8",
    "--recreate",
  ]);
  match command {
    extract_commands::osm_pbf_data {
      inputs,
      include_relations,
      include_ways,
      include_nodes,
      ignore_info,
      tags_include_list,
      tags_ignore_list,
      recreate,
      buffer_limit_in_mb,
    } => {
      assert_eq!(inputs, vec!["a.pbf"]);
      assert!(include_relations && include_ways && include_nodes && ignore_info);
      assert_eq!(tags_include_list.as_deref(), Some("name,highway"));
      assert_eq!(tags_ignore_list.as_deref(), Some("bogus"));
      assert!(recreate);
      assert_eq!(buffer_limit_in_mb, Some(8));
    }
    _ => panic!("expected osm-pbf-data"),
  }

  let (_, command) = parse_extract(&[
    "geolite",
    "extract",
    "osm-admin-levels",
    "--admin-level",
    "2,8",
    "--name-priority",
    "name:pt,name",
    "--recreate",
  ]);
  match command {
    extract_commands::osm_admin_levels { admin_level, recreate, name_priority } => {
      assert_eq!(admin_level.as_deref(), Some("2,8"));
      assert_eq!(name_priority.as_deref(), Some("name:pt,name"));
      assert!(recreate);
    }
    _ => panic!("expected osm-admin-levels"),
  }

  let (_, command) = parse_extract(&["geolite", "extract", "osm-house-numbers", "--recreate"]);
  match command {
    extract_commands::osm_house_numbers { recreate } => assert!(recreate),
    _ => panic!("expected osm-house-numbers"),
  }
}

#[test]
fn _02_05_index_parses_default_and_each_subcommand() {
  assert!(matches!(
    parse(&["geolite", "index"]).command,
    commands::index { command: None }
  ));
  assert!(matches!(
    parse(&["geolite", "index", "admin-levels-hierarchy"]).command,
    commands::index { command: Some(index_commands::admin_levels_hierarchy) }
  ));
  assert!(matches!(
    parse(&["geolite", "index", "user-friendly-name"]).command,
    commands::index { command: Some(index_commands::user_friendly_name) }
  ));
  assert!(matches!(
    parse(&["geolite", "index", "coordinates"]).command,
    commands::index { command: Some(index_commands::coordinates) }
  ));
}

#[test]
fn _02_06_optimize_parses_default_and_each_subcommand() {
  assert!(matches!(
    parse(&["geolite", "optimize"]).command,
    commands::optimize { command: None }
  ));
  assert!(matches!(
    parse(&["geolite", "optimize", "delete-intermediary-data"]).command,
    commands::optimize { command: Some(optimize_commands::delete_intermediary_data) }
  ));
  match parse(&["geolite", "optimize", "sqlite-file", "some.sqlite3"]).command {
    commands::optimize { command: Some(optimize_commands::sqlite_file { pbf_or_sqlite }) } => {
      assert_eq!(pbf_or_sqlite, "some.sqlite3");
    }
    _ => panic!("expected optimize sqlite-file"),
  }
}

#[test]
fn _02_07_query_parses_every_option() {
  let args = parse(&[
    "geolite",
    "query",
    "-23.9,-46.3",
    "--friendly-name-format",
    "{admin_level_12_name}, {house_number}",
    "--min-quality",
    "0.5",
    "--bounding-wkt",
    "POLYGON((-47 -24,-46 -24,-46 -23,-47 -23,-47 -24))",
    "--last-admin-levels",
    "8,12",
    "--include-wkt",
    "false",
  ]);
  match args.command {
    commands::query {
      input,
      friendly_name_format,
      min_quality,
      bounding_wkt,
      last_admin_levels,
      include_wkt,
    } => {
      assert_eq!(input, "-23.9,-46.3");
      assert_eq!(
        friendly_name_format.as_deref(),
        Some("{admin_level_12_name}, {house_number}")
      );
      assert_eq!(min_quality, Some(0.5));
      assert!(bounding_wkt.is_some());
      assert_eq!(last_admin_levels, Some(vec![8, 12]));
      assert!(!include_wkt);
    }
    _ => panic!("expected query command"),
  }
}

#[test]
fn _02_08_query_rejects_invalid_value_parser_inputs() {
  for args in [
    ["geolite", "query", "x", "--min-quality", "2"],
    ["geolite", "query", "x", "--friendly-name-format", "{foo}"],
    ["geolite", "query", "x", "--bounding-wkt", "POINT(1 1)"],
  ] {
    assert!(
      cli::try_parse_from(args).is_err(),
      "invalid option value must be rejected: {args:?}"
    );
  }
}

#[test]
fn _02_09_http_server_build_and_merge_parse() {
  match parse(&["geolite", "http-server"]).command {
    commands::http_server { host, port } => {
      assert_eq!(host, "0.0.0.0");
      assert_eq!(port, 8080);
    }
    _ => panic!("expected http-server command"),
  }
  match parse(&["geolite", "http-server", "--host", "127.0.0.1", "--port", "9090"]).command {
    commands::http_server { host, port } => {
      assert_eq!(host, "127.0.0.1");
      assert_eq!(port, 9090);
    }
    _ => panic!("expected http-server command"),
  }
  match parse(&["geolite", "build", "sul"]).command {
    commands::build { source } => assert_eq!(source, "sul"),
    _ => panic!("expected build command"),
  }
  match parse(&["geolite", "merge", "base.sqlite3", "a.sqlite3", "b.sqlite3"]).command {
    commands::merge { base, databases } => {
      assert_eq!(base, "base.sqlite3");
      assert_eq!(databases, vec!["a.sqlite3", "b.sqlite3"]);
    }
    _ => panic!("expected merge command"),
  }
  // the databases positional is a Vec: zero occurrences still parse (the handler exits at
  // runtime instead), which _04-group child tests rely on.
  match parse(&["geolite", "merge", "base.sqlite3"]).command {
    commands::merge { base, databases } => {
      assert_eq!(base, "base.sqlite3");
      assert!(databases.is_empty());
    }
    _ => panic!("expected merge command"),
  }
}

#[test]
fn _03_00_require_sqlite_returns_on_readable_file() {
  let work = temp_base::new("require_sqlite_ok");
  let path = work.base.join("db.sqlite3");
  std::fs::write(&path, b"").expect("failed to create sqlite probe file");
  require_sqlite(&path.to_string_lossy());
}

#[test]
fn _03_01_preflight_checks_pass_on_writable_dir_and_absent_sqlite() {
  let work = temp_base::new("preflight_ok");
  let data = work.base.join("data");
  let data = data.to_string_lossy();
  preflight_checks(&data, &format!("{data}/db.sqlite3"));
  assert!(
    !std::path::Path::new(&format!("{data}/.geolite_preflight")).exists(),
    "preflight probe file must be removed"
  );

  // an existing readable sqlite passes the last check too.
  std::fs::write(format!("{data}/db.sqlite3"), b"").expect("failed to create sqlite file");
  preflight_checks(&data, &format!("{data}/db.sqlite3"));
}

// ---- process::exit paths, asserted from a respawned child ----

#[test]
#[ignore] // executed only as a child of _04_00
fn _90_require_sqlite_missing_file() {
  let path = std::env::temp_dir().join(format!("geolite_missing_{}", unique_tag()));
  require_sqlite(&path.to_string_lossy());
}

#[test]
fn _04_00_require_sqlite_missing_file_exits_one() {
  let out = respawn("cli::tests::_90_require_sqlite_missing_file", &[], &[]);
  assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr_of(&out));
  assert!(stderr_of(&out).contains("sqlite not found"));
}

#[test]
#[ignore] // executed only as a child of _04_01
fn _90_require_sqlite_unreadable_file() {
  let path = std::env::var("GEOLITE_TEST_PATH").expect("GEOLITE_TEST_PATH");
  require_sqlite(&path);
}

#[cfg(unix)]
#[test]
fn _04_01_require_sqlite_unreadable_file_exits_one() {
  let work = temp_base::new("require_sqlite_unreadable");
  let path = work.base.join("db.sqlite3");
  std::fs::write(&path, b"").expect("failed to create sqlite file");
  chmod(&path, 0o200);
  if std::fs::File::open(&path).is_ok() {
    eprintln!("skipping: running as root, chmod 0o200 is not effective");
    return;
  }
  let out = respawn(
    "cli::tests::_90_require_sqlite_unreadable_file",
    &[("GEOLITE_TEST_PATH", &path.to_string_lossy())],
    &[],
  );
  assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr_of(&out));
  assert!(stderr_of(&out).contains("sqlite not readable"));
}

#[test]
#[ignore] // executed only as a child of the _04_02.._04_05 preflight tests
fn _90_preflight_checks_from_env() {
  let data = std::env::var("GEOLITE_TEST_DATA_PATH").expect("GEOLITE_TEST_DATA_PATH");
  let sqlite = std::env::var("GEOLITE_TEST_SQLITE_PATH").expect("GEOLITE_TEST_SQLITE_PATH");
  preflight_checks(&data, &sqlite);
}

fn respawn_preflight(data_path: &str, sqlite_path: &str) -> std::process::Output {
  respawn(
    "cli::tests::_90_preflight_checks_from_env",
    &[
      ("GEOLITE_TEST_DATA_PATH", data_path),
      ("GEOLITE_TEST_SQLITE_PATH", sqlite_path),
    ],
    &[],
  )
}

#[cfg(unix)]
#[test]
fn _04_02_preflight_uncreatable_data_dir_exits_two() {
  let work = temp_base::new("preflight_uncreatable");
  let parent = work.base.join("locked");
  std::fs::create_dir_all(&parent).expect("failed to create locked dir");
  chmod(&parent, 0o555);
  let data = parent.join("sub");
  if std::fs::create_dir_all(&data).is_ok() {
    eprintln!("skipping: running as root, chmod 0o555 is not effective");
    return;
  }
  let data = data.to_string_lossy();
  let out = respawn_preflight(&data, &format!("{data}/db.sqlite3"));
  assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr_of(&out));
  assert!(stderr_of(&out).contains("failed to create dir"));
}

#[cfg(unix)]
#[test]
fn _04_03_preflight_unwritable_data_dir_exits_two() {
  let work = temp_base::new("preflight_unwritable");
  let data = work.base.join("data");
  std::fs::create_dir_all(&data).expect("failed to create data dir");
  chmod(&data, 0o555);
  if std::fs::write(data.join("probe"), b"").is_ok() {
    eprintln!("skipping: running as root, chmod 0o555 is not effective");
    return;
  }
  let data = data.to_string_lossy();
  let out = respawn_preflight(&data, &format!("{data}/db.sqlite3"));
  assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr_of(&out));
  assert!(stderr_of(&out).contains("no write access to dir"));
}

#[cfg(unix)]
#[test]
fn _04_04_preflight_unreadable_data_dir_exits_one() {
  let work = temp_base::new("preflight_unreadable_dir");
  let data = work.base.join("data");
  std::fs::create_dir_all(&data).expect("failed to create data dir");
  // 0o333: probe write succeeds (w+x), read_dir fails (no r).
  chmod(&data, 0o333);
  if std::fs::read_dir(&data).is_ok() {
    eprintln!("skipping: running as root, chmod 0o333 is not effective");
    return;
  }
  let data = data.to_string_lossy();
  let out = respawn_preflight(&data, &format!("{data}/db.sqlite3"));
  assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr_of(&out));
  assert!(stderr_of(&out).contains("no read access to dir"));
}

#[cfg(unix)]
#[test]
fn _04_05_preflight_unreadable_sqlite_exits_one() {
  let work = temp_base::new("preflight_unreadable_sqlite");
  let data = work.base.join("data");
  std::fs::create_dir_all(&data).expect("failed to create data dir");
  let sqlite = data.join("db.sqlite3");
  std::fs::write(&sqlite, b"").expect("failed to create sqlite file");
  chmod(&sqlite, 0o000);
  if std::fs::File::open(&sqlite).is_ok() {
    eprintln!("skipping: running as root, chmod 0o000 is not effective");
    return;
  }
  let out = respawn_preflight(&data.to_string_lossy(), &sqlite.to_string_lossy());
  assert_eq!(out.status.code(), Some(1), "stderr: {}", stderr_of(&out));
  assert!(stderr_of(&out).contains("no read access to file"));
}

#[test]
#[ignore] // executed only as a child of _04_06
fn _90_default_data_dir_without_home() {
  assert_eq!(super::default_data_dir(), "./.geolite-data");
}

#[test]
fn _04_06_default_data_dir_falls_back_when_home_is_unset() {
  let out = respawn("cli::tests::_90_default_data_dir_without_home", &[], &["HOME"]);
  assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr_of(&out));
}
