use super::{coalesce_of, equals, is_not_null, is_null, normalized_coalesce, not_in};
use crate::domain::osm_tag::{key, osm_tag};

#[test]
fn _00_builds_an_equality_predicate() {
  assert_eq!(
    equals("payload", osm_tag::place, "suburb"),
    "JSON_EXTRACT(payload, '$.tags.\"place\"') = 'suburb'"
  );
}

#[test]
fn _01_treats_a_missing_tag_as_empty_when_excluding() {
  assert_eq!(
    not_in("payload", osm_tag::leisure, &["park", "garden"]),
    "COALESCE(JSON_EXTRACT(payload, '$.tags.\"leisure\"'), '') NOT IN ('park', 'garden')"
  );
}

#[test]
fn _02_builds_the_null_checks() {
  assert_eq!(
    is_null("payload", osm_tag::building),
    "JSON_EXTRACT(payload, '$.tags.\"building\"') IS NULL"
  );
  assert_eq!(
    is_not_null("payload", osm_tag::name),
    "JSON_EXTRACT(payload, '$.tags.\"name\"') IS NOT NULL"
  );
}

#[test]
fn _03_escapes_a_quote_inside_a_value() {
  assert_eq!(
    equals("payload", osm_tag::name, "Rua d'Alfândega"),
    "JSON_EXTRACT(payload, '$.tags.\"name\"') = 'Rua d''Alfândega'"
  );
}

#[test]
fn _04_skips_coalesce_for_a_single_key() {
  assert_eq!(
    coalesce_of("payload", &["name"]),
    "JSON_EXTRACT(payload, '$.tags.\"name\"')"
  );
}

#[test]
fn _05_falls_back_in_the_order_given() {
  assert_eq!(
    coalesce_of("payload", &["name:pt", "name"]),
    "COALESCE(JSON_EXTRACT(payload, '$.tags.\"name:pt\"'), JSON_EXTRACT(payload, '$.tags.\"name\"'))"
  );
}

#[test]
fn _06_falls_back_to_the_name_tag_when_no_key_is_given() {
  assert_eq!(
    coalesce_of("payload", &[]),
    "JSON_EXTRACT(payload, '$.tags.\"name\"')"
  );
}

#[test]
fn _07_normalises_an_alias_group() {
  assert_eq!(
    normalized_coalesce("payload", key::POST_CODE),
    "NULLIF(UPPER(TRIM(COALESCE(JSON_EXTRACT(payload, '$.tags.\"postal_code\"'), JSON_EXTRACT(payload, '$.tags.\"addr:postcode\"')))), '')"
  );
}

#[test]
fn _08_runs_against_sqlite() {
  let conn = rusqlite::Connection::open_in_memory().expect("failed to open sqlite");
  conn
    .execute_batch(
      "CREATE TABLE t(payload BLOB);
       INSERT INTO t VALUES (jsonb('{\"tags\":{\"name\":\"Rua Alfa\",\"name:pt\":\"Rua Beta\",\"addr:postcode\":\" 1100-001 \"}}'));
       INSERT INTO t VALUES (jsonb('{\"tags\":{\"name\":\"Rua Gama\",\"leisure\":\"park\"}}'));",
    )
    .expect("failed to seed");
  let select = |columns: String, filter: String| -> Vec<(String, Option<String>)> {
    conn
      .prepare(&format!("SELECT {columns} FROM t WHERE {filter} ORDER BY rowid"))
      .expect("failed to prepare")
      .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
      .expect("failed to query")
      .collect::<Result<_, _>>()
      .expect("failed to collect")
  };
  let columns = format!(
    "{}, {}",
    coalesce_of("payload", &["name:pt", "name"]),
    normalized_coalesce("payload", key::POST_CODE)
  );

  assert_eq!(
    select(columns.clone(), is_not_null("payload", osm_tag::name)),
    vec![
      ("Rua Beta".to_string(), Some("1100-001".to_string())),
      ("Rua Gama".to_string(), None)
    ]
  );
  assert_eq!(
    select(columns.clone(), not_in("payload", osm_tag::leisure, &["park"])),
    vec![("Rua Beta".to_string(), Some("1100-001".to_string()))]
  );
  assert_eq!(
    select(columns, equals("payload", osm_tag::leisure, "park")),
    vec![("Rua Gama".to_string(), None)]
  );
}
