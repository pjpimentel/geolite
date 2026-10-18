use crate::common::harness::{open_sqlite_at, plain, scenario, world, world_cell};

pub static SCENARIO: scenario = scenario {
  region: "01_general",
  fixture_dir: "01_general",
  pbf: "south-america.osm.pbf",
};

pub(crate) static WORLD: world_cell = world_cell::new();

pub(crate) fn world() -> &'static world {
  WORLD.get(&SCENARIO)
}

const ISOS: [&str; 13] = [
  "ar", "bo", "br", "cl", "co", "ec", "gf", "gy", "pe", "py", "sr", "uy", "ve",
];

// 00.00. a source holding one boundary and one street in each of the thirteen countries of south
// america builds one base per country, each one holding its country and its street under its own
// iso, sealed, with nothing else left in the data path
#[test]
#[ignore]
fn _00_00_a_build_of_south_america_writes_one_base_per_country() {
  const SQL_SELECT_LEVELS: &str = "
    SELECT
      admin_level,
      country_iso_code
    FROM admin_levels
    ORDER BY admin_level
  ";

  let w = world();
  assert_eq!(w.build_status, 0, "{}", w.build_stderr);

  let mut folders: Vec<String> = std::fs::read_dir(&w.data_path)
    .expect("failed to list the data path")
    .flatten()
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .collect();
  folders.sort();
  assert_eq!(folders, ISOS, "one folder per country and nothing else");

  for iso in ISOS {
    let folder = w.data_path.join(iso);
    assert!(folder.join("data.sqlite3").is_file(), "{iso} has no base");
    assert!(
      folder.join("index.tantivy").join("meta.json").is_file(),
      "{iso} has no index"
    );
    let conn = open_sqlite_at(&folder.join("data.sqlite3"));
    let rows: Vec<(u8, String)> = conn
      .prepare(SQL_SELECT_LEVELS)
      .expect("failed to prepare")
      .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
      .expect("failed to query")
      .map(|r| r.expect("failed to read a row"))
      .collect();
    let country = iso.to_ascii_uppercase();
    assert_eq!(
      rows,
      vec![(2, country.clone()), (12, country)],
      "{iso} holds its country and its street, and nothing else"
    );
  }

  let sealed = format!(
    "sealed {} — read-only",
    ISOS
      .iter()
      .map(|iso| format!("{iso}/data.sqlite3, {iso}/index.tantivy"))
      .collect::<Vec<_>>()
      .join(", ")
  );
  assert!(
    plain(&w.build_stdout).contains(&sealed),
    "{}",
    w.build_stdout
  );
}
