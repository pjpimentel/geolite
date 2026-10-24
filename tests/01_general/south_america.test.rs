use crate::common::harness::{open_sqlite_at, plain, scenario, world, world_cell};
use crate::common::query::{countries_of, files_of, first, matches};
use serde_json::Value;

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
const ARGENTINA_STREET: &str = "-32,-64";
const PANAMA_CITY: &str = "8.98,-79.52";

fn similarities_of(answer: &Value) -> Vec<f64> {
  matches(answer)
    .iter()
    .map(|m| {
      m["similarity"]
        .as_f64()
        .expect("a text answers a similarity")
    })
    .collect()
}

fn assert_only(answer: &Value, iso: &str) {
  let countries = countries_of(answer);
  assert!(
    !countries.is_empty() && countries.iter().all(|c| c == iso),
    "{iso} alone answers: {countries:?}"
  );
}

fn assert_nothing(answer: &Value) {
  assert!(files_of(answer).is_empty(), "no base is named: {answer}");
  assert!(matches(answer).is_empty(), "nothing answers: {answer}");
}

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

// 00.01. contract: a text one base holds exactly is answered by that base alone: the exact query
// runs on every base at once and the fuzzy one of the twelve others never runs, so no
// `<country> street` at half coverage comes beside `argentina street`
#[test]
#[ignore]
fn _00_01_a_text_one_base_holds_exactly_is_answered_by_that_base_alone() {
  let w = world();
  let answer = w.run(&["argentina street"]);
  assert_eq!(
    files_of(&answer).len(),
    ISOS.len(),
    "every base is asked: {answer}"
  );
  assert_only(&answer, "AR");
  let similarities = similarities_of(&answer);
  assert!(similarities.iter().all(|s| *s == 1.0), "{similarities:?}");
}

// 00.02. contract: a text no base holds exactly is answered by every base through the loose
// query, joined by the coverage of the tokens: `argentina stret` covers half of the query in
// argentina and nothing exactly elsewhere, so argentina leads and the others follow
#[test]
#[ignore]
fn _00_02_a_text_no_base_holds_exactly_is_answered_by_every_base_by_coverage() {
  let w = world();
  let answer = w.run(&["argentina stret"]);
  let countries = countries_of(&answer);
  assert_eq!(
    countries.first().map(String::as_str),
    Some("AR"),
    "{countries:?}"
  );
  assert!(
    countries.iter().any(|iso| iso != "AR"),
    "the other bases answer too: {countries:?}"
  );
  let similarities = similarities_of(&answer);
  assert_eq!(similarities[0], 0.5, "{similarities:?}");
  assert!(
    similarities.windows(2).all(|pair| pair[0] >= pair[1]),
    "joined by coverage: {similarities:?}"
  );
}

// 00.03. contract: a coordinate is answered by the base of the country whose boundary holds the
// point, named alone in `databases`; a country `--countries` leaves out, or one without a base,
// answers nothing and names no base
#[test]
#[ignore]
fn _00_03_a_coordinate_is_answered_by_the_base_of_its_country_alone() {
  let w = world();
  let answer = w.run(&[ARGENTINA_STREET]);
  assert_eq!(files_of(&answer), ["ar/data.sqlite3"]);
  assert_only(&answer, "AR");
  assert!(
    first(&answer)["friendly_name"]
      .as_str()
      .unwrap_or_default()
      .starts_with("Argentina Street"),
    "{answer}"
  );
  assert_nothing(&w.run(&[ARGENTINA_STREET, "--countries", "br"]));
  assert_nothing(&w.run(&[PANAMA_CITY]));
}
