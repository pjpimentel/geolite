use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::common::harness::{open_sqlite_at, output, world};
use crate::general::world;
use serde_json::{Value, json};

const NODES: i64 = 683_313;
const WAYS: i64 = 41_335;
const RELATIONS: i64 = 1_274;
const DATA_CHUNKS: i64 = 93;
const CHUNKS: i64 = DATA_CHUNKS + 1;

const COUNTRY: u64 = 59_470;
const STATE: u64 = 298_204;
const MUNICIPALITY: u64 = 298_442;
const STREET_WAY: u64 = 169_924_327;
const STREET_NODE: u64 = 1_785_552_339;

const REGENERATE: &str = "the fixture changed; regenerate deliberately and update the constants";

struct scratch {
  dir: PathBuf,
  stdout: String,
}

impl scratch {
  fn ledger(&self) -> rusqlite::Connection {
    open_sqlite_at(&self.dir.join("database.sqlite3"))
  }

  fn osm_data(&self) -> rusqlite::Connection {
    open_sqlite_at(&self.dir.join("database.osm_data.sqlite3"))
  }

  fn pbf(&self) -> String {
    self
      .dir
      .join("santos.osm.pbf")
      .to_string_lossy()
      .into_owned()
  }
}

struct ledger_row {
  id: i64,
  origin: u8,
  node_count: Option<i64>,
  way_count: Option<i64>,
  relation_count: Option<i64>,
  extracted_at: Option<i64>,
}

impl ledger_row {
  fn counts(&self) -> (Option<i64>, Option<i64>, Option<i64>) {
    (self.node_count, self.way_count, self.relation_count)
  }
}

fn stage(w: &world, dir: &Path, args: &[&str]) -> output {
  let out = w.geolite_in(dir, args);
  assert_eq!(
    out.status, 0,
    "geolite {args:?} exited {}:\n{}",
    out.status, out.stderr
  );
  out
}

fn copy_fixture(w: &world, dir: &Path, name: &str) -> String {
  let target = dir.join(name);
  std::fs::copy(&w.pbf, &target).expect("failed to copy the fixture");
  target.to_string_lossy().into_owned()
}

fn osm_data(w: &world, dir: &Path, threads: &str, data_args: &[&str], inputs: &[&str]) -> output {
  let mut args = vec!["--threads", threads, "extract", "osm-pbf-data"];
  args.extend_from_slice(data_args);
  args.extend_from_slice(inputs);
  stage(w, dir, &args)
}

// the three stages of the file in a scratch of its own, keeping the osm-data stdout
fn extracted(w: &world, name: &str, threads: &str, data_args: &[&str]) -> scratch {
  let dir = w.scratch(name);
  let pbf = copy_fixture(w, &dir, "santos.osm.pbf");
  stage(w, &dir, &["extract", "osm-pbf-blob-chunks", &pbf]);
  stage(w, &dir, &["extract", "osm-pbf-header", &pbf]);
  let stdout = osm_data(w, &dir, threads, data_args, &[&pbf]).stdout;
  scratch { dir, stdout }
}

fn admin_levels(w: &world, dir: &Path, extra: &[&str]) {
  let mut args = vec![
    "--preset",
    "brazil",
    "extract",
    "osm-admin-levels",
    "--admin-level",
    "2,4,8",
  ];
  args.extend_from_slice(extra);
  stage(w, dir, &args);
}

fn count(conn: &rusqlite::Connection, sql: &str) -> i64 {
  conn
    .query_row(sql, [], |r| r.get(0))
    .unwrap_or_else(|e| panic!("{sql} failed: {e}"))
}

fn table_counts(conn: &rusqlite::Connection) -> (i64, i64, i64) {
  (
    count(conn, "SELECT COUNT(*) FROM osm_nodes"),
    count(conn, "SELECT COUNT(*) FROM osm_ways"),
    count(conn, "SELECT COUNT(*) FROM osm_relations"),
  )
}

fn payload(conn: &rusqlite::Connection, table: &str, id: u64) -> Value {
  let text: String = conn
    .query_row(
      &format!("SELECT JSON(payload) FROM {table} WHERE id = ?1"),
      [id],
      |r| r.get(0),
    )
    .unwrap_or_else(|e| panic!("{table} has no row {id}: {e}"));
  serde_json::from_str(&text).expect("the payload must be json")
}

// count, sum of ids and payload bytes: equal signatures mean equal tables for this fixture
fn signature(conn: &rusqlite::Connection, table: &str) -> (i64, i64, i64) {
  conn
    .query_row(
      &format!(
        "SELECT COUNT(*), COALESCE(SUM(id), 0), COALESCE(SUM(LENGTH(payload)), 0) FROM {table}"
      ),
      [],
      |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .expect("failed to sign the table")
}

fn ledger(conn: &rusqlite::Connection) -> Vec<ledger_row> {
  conn
    .prepare(
      "SELECT id, origin, node_count, way_count, relation_count, osm_data_extracted_at \
       FROM osm_pbf_files ORDER BY id",
    )
    .expect("failed to prepare the ledger query")
    .query_map([], |r| {
      Ok(ledger_row {
        id: r.get(0)?,
        origin: r.get(1)?,
        node_count: r.get(2)?,
        way_count: r.get(3)?,
        relation_count: r.get(4)?,
        extracted_at: r.get(5)?,
      })
    })
    .expect("failed to read the ledger")
    .collect::<Result<Vec<_>, _>>()
    .expect("failed to collect the ledger")
}

fn boundary(conn: &rusqlite::Connection, relation_id: u64) -> (u8, String, Option<String>) {
  conn
    .query_row(
      "SELECT admin_level, name, country_iso_code FROM admin_levels WHERE relation_id = ?1",
      [relation_id],
      |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap_or_else(|e| panic!("relation {relation_id} is missing from admin_levels: {e}"))
}

fn assert_full_fixture(s: &scratch) {
  assert_eq!(
    table_counts(&s.osm_data()),
    (NODES, WAYS, RELATIONS),
    "{REGENERATE}"
  );
  let rows = ledger(&s.ledger());
  assert_eq!(rows.len(), 1, "one file, one ledger row");
  assert_eq!(rows[0].counts(), (Some(NODES), Some(WAYS), Some(RELATIONS)));
  assert!(
    rows[0].extracted_at.is_some(),
    "the stage must stamp the ledger"
  );
}

fn assert_same_tables(a: &scratch, b: &scratch) {
  for table in ["osm_nodes", "osm_ways", "osm_relations"] {
    assert_eq!(
      signature(&a.osm_data(), table),
      signature(&b.osm_data(), table),
      "{table} differs between the two extractions"
    );
  }
}

// 00. the stage on the real fixture
#[test]
#[ignore]
fn _00_the_osm_data_stage_writes_the_fixture_exactly() {
  let w = world();
  let s = extracted(w, "extract_exact", "2", &[]);
  assert_full_fixture(&s);
  for line in [
    "done (94 chunks)",
    "santos.osm.pbf  nodes: 683.3K  ways: 41.3K  relations: 1.3K",
    " osm_nodes in ",
  ] {
    assert!(
      s.stdout.contains(line),
      "stdout lacks {line:?}:\n{}",
      s.stdout
    );
  }
}

// 01. the chunk is the link between the file and every row
#[test]
#[ignore]
fn _01_every_element_row_points_at_a_data_chunk_of_its_file() {
  let w = world();
  let s = extracted(w, "extract_chunk_links", "2", &[]);
  let data = s.osm_data();
  let file_id = ledger(&s.ledger())[0].id;
  for table in ["osm_nodes", "osm_ways", "osm_relations"] {
    let strays = count(
      &data,
      &format!(
        "SELECT COUNT(*) FROM {table} t \
         LEFT JOIN osm_pbf_blob_chunks c ON c.id = t.osm_pbf_chunk_id \
         WHERE c.id IS NULL OR c.chunk_type = 0 OR c.file_id != {file_id}"
      ),
    );
    assert_eq!(
      strays, 0,
      "{table} rows must point at a data chunk of file {file_id}"
    );
  }
  let used = count(
    &data,
    "SELECT COUNT(*) FROM (SELECT osm_pbf_chunk_id FROM osm_nodes \
     UNION SELECT osm_pbf_chunk_id FROM osm_ways \
     UNION SELECT osm_pbf_chunk_id FROM osm_relations)",
  );
  assert_eq!(
    used, DATA_CHUNKS,
    "every data chunk of the fixture yields rows; {REGENERATE}"
  );
  assert_eq!(
    count(
      &data,
      &format!(
        "SELECT COUNT(*) FROM osm_pbf_blob_chunks WHERE file_id = {file_id} AND chunk_type = 1"
      )
    ),
    DATA_CHUNKS
  );
  for index in [
    "osm_nodes_search_by_chunk",
    "osm_ways_search_by_chunk",
    "osm_relations_search_by_admin_level",
  ] {
    assert_eq!(
      count(
        &data,
        &format!("SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = '{index}'")
      ),
      1,
      "{index} must exist after the stage"
    );
  }
}

// 02. decoders and jsonb on real elements
#[test]
#[ignore]
fn _02_known_elements_keep_their_tags_references_and_coordinates() {
  let w = world();
  let s = extracted(w, "extract_payloads", "2", &[]);
  let data = s.osm_data();
  assert_eq!(
    payload(&data, "osm_nodes", STREET_NODE),
    json!({"lat": -23.8759538, "lon": -46.4166851, "tags": {}})
  );
  assert_eq!(
    payload(&data, "osm_ways", STREET_WAY),
    json!({
      "refs": [1_785_552_339u64, 1_785_552_054u64],
      "tags": {"name": "Rua Castro Alves", "highway": "residential"}
    })
  );
  let country = payload(&data, "osm_relations", COUNTRY);
  let tags = country["tags"]
    .as_object()
    .expect("the relation has a tags object");
  assert_eq!(tags.len(), 501, "{REGENERATE}");
  assert_eq!(tags["admin_level"], "2");
  assert_eq!(tags["ISO3166-1:alpha2"], "BR");
  let members = country["members"]
    .as_array()
    .expect("the relation has a members array");
  assert_eq!(members.len(), 1_211, "{REGENERATE}");
  let of_type = |t: &str| members.iter().filter(|m| m["type"] == t).count();
  assert_eq!((of_type("w"), of_type("n"), of_type("r")), (1_177, 2, 32));
  assert_eq!(
    members
      .iter()
      .filter(|m| m["type"] == "w" && m["role"] == "outer")
      .count(),
    1_177,
    "every way member of the country is an outer ring segment"
  );
}

// 03. the tag policy reaches the stage after
#[test]
#[ignore]
fn _03_a_tag_include_list_keeps_only_the_listed_keys() {
  let w = world();
  let s = extracted(
    w,
    "extract_tag_include",
    "2",
    &["--tags-include-list", "name,admin_level"],
  );
  let data = s.osm_data();
  for table in ["osm_nodes", "osm_ways", "osm_relations"] {
    let foreign = count(
      &data,
      &format!(
        "SELECT COUNT(*) FROM {table}, JSON_EACH(JSON_EXTRACT(payload, '$.tags')) AS tag \
         WHERE tag.key NOT IN ('name', 'admin_level')"
      ),
    );
    assert_eq!(foreign, 0, "{table} carries a key outside the include list");
  }
  assert_eq!(
    payload(&data, "osm_relations", COUNTRY)["tags"]
      .as_object()
      .map(|t| t.len()),
    Some(2)
  );
  assert_eq!(
    count(
      &data,
      "SELECT COUNT(*) FROM osm_ways WHERE JSON_EXTRACT(payload, '$.tags.name') IS NOT NULL"
    ),
    17_695,
    "{REGENERATE}"
  );
  admin_levels(w, &s.dir, &[]);
  let conn = s.ledger();
  assert_eq!(boundary(&conn, COUNTRY), (2, "Brasil".to_string(), None));
  assert_eq!(boundary(&conn, STATE), (4, "São Paulo".to_string(), None));
  assert_eq!(
    boundary(&conn, MUNICIPALITY),
    (8, "Santos".to_string(), None)
  );
}

// 04. the tag policy can starve the stage after
#[test]
#[ignore]
fn _04_a_tag_ignore_list_dropping_name_starves_the_admin_level_stage() {
  let w = world();
  let s = extracted(
    w,
    "extract_tag_ignore",
    "2",
    &["--tags-ignore-list", "name"],
  );
  assert_eq!(
    count(
      &s.osm_data(),
      "SELECT COUNT(*) FROM osm_ways WHERE JSON_EXTRACT(payload, '$.tags.name') IS NOT NULL"
    ),
    0
  );
  admin_levels(w, &s.dir, &[]);
  assert_eq!(
    count(&s.ledger(), "SELECT COUNT(*) FROM admin_levels"),
    0,
    "without names there is no candidate at any level"
  );
}

// 05. coalesce_of through the binary
#[test]
#[ignore]
fn _05_name_priority_falls_back_key_by_key() {
  let w = world();
  let s = extracted(w, "extract_name_priority", "2", &[]);
  admin_levels(w, &s.dir, &["--name-priority", "name:en,name"]);
  let conn = s.ledger();
  assert_eq!(
    boundary(&conn, COUNTRY),
    (2, "Brazil".to_string(), Some("BR".to_string()))
  );
  assert_eq!(boundary(&conn, STATE), (4, "São Paulo".to_string(), None));
  assert_eq!(
    boundary(&conn, MUNICIPALITY),
    (8, "Santos".to_string(), None),
    "no name:en on the municipality, so the priority falls back to name"
  );
}

// 06. the stage refuses to run before the chunk index and leaves the ledger untouched
#[test]
#[ignore]
fn _06_the_stage_before_the_chunk_index_stamps_nothing() {
  let w = world();
  let dir = w.scratch("extract_without_chunks");
  let pbf = copy_fixture(w, &dir, "santos.osm.pbf");
  let out = osm_data(w, &dir, "2", &[], &[&pbf]);
  assert!(
    out
      .stderr
      .contains("no blob chunks found — run extract osm-pbf-blob-chunks first"),
    "stderr: {}",
    out.stderr
  );
  let rows = ledger(&open_sqlite_at(&dir.join("database.sqlite3")));
  assert_eq!(rows.len(), 1, "resolving the input records the file");
  assert_eq!(rows[0].origin, 0);
  assert_eq!(
    (rows[0].counts(), rows[0].extracted_at),
    ((None, None, None), None),
    "nothing decoded, nothing stamped"
  );
  assert_eq!(
    table_counts(&open_sqlite_at(&dir.join("database.osm_data.sqlite3"))),
    (0, 0, 0)
  );
  stage(w, &dir, &["extract", "osm-pbf-blob-chunks", &pbf]);
  let stdout = osm_data(w, &dir, "2", &[], &[&pbf]).stdout;
  assert_full_fixture(&scratch { dir, stdout });
}

// 07. the pipeline is deterministic under concurrency
#[test]
#[ignore]
fn _07_the_thread_count_does_not_change_the_result() {
  let w = world();
  let single = extracted(w, "extract_one_thread", "1", &[]);
  let many = extracted(w, "extract_four_threads", "4", &[]);
  assert_same_tables(&single, &many);
  assert_eq!(
    payload(&single.osm_data(), "osm_ways", STREET_WAY),
    payload(&many.osm_data(), "osm_ways", STREET_WAY)
  );
}

// 08. backpressure at real volume loses nothing
#[test]
#[ignore]
fn _08_a_one_megabyte_buffer_gives_the_same_result() {
  let w = world();
  let roomy = extracted(w, "extract_default_buffer", "2", &[]);
  let tight = extracted(
    w,
    "extract_tight_buffer",
    "2",
    &["--buffer-limit-in-mb", "1"],
  );
  assert_same_tables(&roomy, &tight);
  assert!(
    tight
      .stdout
      .contains("santos.osm.pbf  nodes: 683.3K  ways: 41.3K  relations: 1.3K"),
    "stdout: {}",
    tight.stdout
  );
}

// 09. one invocation, two files
#[test]
#[ignore]
fn _09_two_inputs_in_one_run_get_their_own_ledger_rows_and_chunks() {
  let w = world();
  let dir = w.scratch("extract_two_inputs");
  let santos = copy_fixture(w, &dir, "santos.osm.pbf");
  let alpha = copy_fixture(w, &dir, "alpha.osm.pbf");
  stage(
    w,
    &dir,
    &["extract", "osm-pbf-blob-chunks", &santos, &alpha],
  );
  osm_data(w, &dir, "2", &[], &[&santos, &alpha]);
  let rows = ledger(&open_sqlite_at(&dir.join("database.sqlite3")));
  assert_eq!(rows.len(), 2, "one ledger row per input");
  let data = open_sqlite_at(&dir.join("database.osm_data.sqlite3"));
  for row in &rows {
    assert_eq!(
      row.counts(),
      (Some(NODES), Some(WAYS), Some(RELATIONS)),
      "each file decodes the whole fixture"
    );
    assert!(row.extracted_at.is_some());
    assert_eq!(
      count(
        &data,
        &format!(
          "SELECT COUNT(*) FROM osm_pbf_blob_chunks WHERE file_id = {}",
          row.id
        )
      ),
      CHUNKS
    );
  }
  assert_eq!(
    table_counts(&data),
    (NODES, WAYS, RELATIONS),
    "the second file's ids are already there and are ignored"
  );
  assert_eq!(
    count(
      &data,
      &format!(
        "SELECT COUNT(*) FROM osm_nodes n JOIN osm_pbf_blob_chunks c ON c.id = n.osm_pbf_chunk_id \
         WHERE c.file_id != {}",
        rows[0].id
      )
    ),
    0,
    "every row belongs to the chunks of the first file"
  );
}

// 10. the stage is idempotent and the stamp is a clock
#[test]
#[ignore]
fn _10_re_running_the_stage_keeps_the_rows_and_refreshes_the_stamp() {
  let w = world();
  let s = extracted(w, "extract_rerun", "2", &[]);
  let before = ledger(&s.ledger())[0]
    .extracted_at
    .expect("the first run stamps the ledger");
  std::thread::sleep(Duration::from_millis(1_100));
  osm_data(w, &s.dir, "2", &[], &[&s.pbf()]);
  assert_full_fixture(&s);
  let after = ledger(&s.ledger())[0]
    .extracted_at
    .expect("the second run stamps the ledger");
  assert!(
    after > before,
    "the stamp must move forward: {before} -> {after}"
  );
}

// 11. the selection flags, now that they take a value
#[test]
#[ignore]
fn _11_the_include_flags_select_what_is_extracted() {
  let w = world();
  let no_nodes = extracted(
    w,
    "extract_without_nodes",
    "2",
    &["--include-nodes", "false"],
  );
  assert_eq!(table_counts(&no_nodes.osm_data()), (0, WAYS, RELATIONS));
  assert_eq!(
    ledger(&no_nodes.ledger())[0].counts(),
    (Some(0), Some(WAYS), Some(RELATIONS))
  );
  let only_nodes = extracted(
    w,
    "extract_only_nodes",
    "2",
    &["--include-ways", "false", "--include-relations", "false"],
  );
  assert_eq!(table_counts(&only_nodes.osm_data()), (NODES, 0, 0));
  assert_eq!(
    ledger(&only_nodes.ledger())[0].counts(),
    (Some(NODES), Some(0), Some(0))
  );
}

// 12. --recreate empties the element tables and keeps the chunk index
#[test]
#[ignore]
fn _12_recreate_rebuilds_the_element_tables_without_touching_the_chunk_index() {
  let w = world();
  let s = extracted(w, "extract_recreate", "2", &[]);
  let out = osm_data(w, &s.dir, "2", &["--recreate"], &[&s.pbf()]);
  assert!(
    out.stdout.contains("done (94 chunks)"),
    "the chunk index must survive --recreate:\n{}",
    out.stdout
  );
  assert!(s.dir.join("database.osm_data.sqlite3").exists());
  assert_eq!(
    count(&s.osm_data(), "SELECT COUNT(*) FROM osm_pbf_blob_chunks"),
    CHUNKS
  );
  assert_full_fixture(&s);
}

// 13. the scale is closed at the cli edge
#[test]
#[ignore]
fn _13_a_level_outside_the_scale_is_refused_by_the_cli() {
  let w = world();
  let s = extracted(w, "extract_unknown_level", "2", &[]);
  let out = w.geolite_in(
    &s.dir,
    &[
      "--preset",
      "brazil",
      "extract",
      "osm-admin-levels",
      "--admin-level",
      "2,11",
    ],
  );
  assert_eq!(
    out.status, 1,
    "an unsupported level must exit with 1:\n{}",
    out.stderr
  );
  assert!(
    out
      .stderr
      .contains("invalid --admin-level: level 11 is not supported"),
    "stderr: {}",
    out.stderr
  );
  assert_eq!(
    count(&s.ledger(), "SELECT COUNT(*) FROM admin_levels"),
    0,
    "nothing is extracted when the list is refused"
  );
}
