use super::coalesce_of;

#[test]
fn _00_skips_coalesce_for_a_single_key() {
  assert_eq!(
    coalesce_of("payload", &["name"]),
    "JSON_EXTRACT(payload, '$.tags.\"name\"')"
  );
}

#[test]
fn _01_falls_back_in_the_order_given() {
  assert_eq!(
    coalesce_of("payload", &["name:pt", "name"]),
    "COALESCE(JSON_EXTRACT(payload, '$.tags.\"name:pt\"'), JSON_EXTRACT(payload, '$.tags.\"name\"'))"
  );
}

#[test]
fn _02_falls_back_to_the_name_tag_when_no_key_is_given() {
  assert_eq!(
    coalesce_of("payload", &[]),
    "JSON_EXTRACT(payload, '$.tags.\"name\"')"
  );
}

#[test]
fn _03_runs_against_sqlite() {
  let conn = rusqlite::Connection::open_in_memory().expect("failed to open sqlite");
  conn
    .execute_batch(
      "CREATE TABLE t(payload BLOB);
       INSERT INTO t VALUES (jsonb('{\"tags\":{\"name\":\"Rua Alfa\",\"name:pt\":\"Rua Beta\"}}'));
       INSERT INTO t VALUES (jsonb('{\"tags\":{\"name\":\"Rua Gama\"}}'));",
    )
    .expect("failed to seed");

  let names: Vec<String> = conn
    .prepare(&format!(
      "SELECT {} FROM t ORDER BY rowid",
      coalesce_of("payload", &["name:pt", "name"])
    ))
    .expect("failed to prepare")
    .query_map([], |row| row.get(0))
    .expect("failed to query")
    .collect::<Result<_, _>>()
    .expect("failed to collect");
  assert_eq!(names, vec!["Rua Beta", "Rua Gama"]);
}
