use super::super::key;
use super::super::value::{highway_value, place_value};
use super::*;

#[test]
fn _00_builds_an_equality_predicate() {
  assert_eq!(
    equals("payload", osm_tag::highway, highway_value::residential.value()),
    "JSON_EXTRACT(payload, '$.tags.\"highway\"') = 'residential'"
  );
}

#[test]
fn _01_treats_a_missing_tag_as_empty_when_excluding() {
  assert_eq!(
    not_in("payload", osm_tag::place, &[place_value::suburb.value()]),
    "COALESCE(JSON_EXTRACT(payload, '$.tags.\"place\"'), '') NOT IN ('suburb')"
  );
}

#[test]
fn _02_builds_null_checks() {
  assert_eq!(
    is_null("payload", osm_tag::building),
    "JSON_EXTRACT(payload, '$.tags.\"building\"') IS NULL"
  );
  assert_eq!(
    is_not_null("osm_data.osm_ways.payload", osm_tag::name),
    "JSON_EXTRACT(osm_data.osm_ways.payload, '$.tags.\"name\"') IS NOT NULL"
  );
}

#[test]
fn _03_skips_coalesce_for_a_single_key() {
  assert_eq!(
    coalesce_of("payload", &["name"]),
    "JSON_EXTRACT(payload, '$.tags.\"name\"')"
  );
}

#[test]
fn _04_falls_back_in_the_order_given() {
  assert_eq!(
    coalesce_of("payload", &["name:pt", "name"]),
    "COALESCE(JSON_EXTRACT(payload, '$.tags.\"name:pt\"'), JSON_EXTRACT(payload, '$.tags.\"name\"'))"
  );
}

#[test]
fn _05_falls_back_to_the_name_tag_when_no_key_is_given() {
  assert_eq!(
    coalesce_of("payload", &[]),
    "JSON_EXTRACT(payload, '$.tags.\"name\"')"
  );
}

#[test]
fn _06_normalises_an_alias_group() {
  let sql = normalized_coalesce("payload", key::COUNTRY_ISO);
  assert!(sql.starts_with("NULLIF(UPPER(TRIM(COALESCE("));
  assert!(sql.ends_with("))), '')"));
  assert!(sql.contains("'$.tags.\"ISO3166-1\"'"));
  assert!(sql.contains("'$.tags.\"ISO3166-1:alpha2\"'"));
}

#[test]
fn _07_escapes_a_quote_inside_a_value() {
  assert_eq!(
    equals("payload", osm_tag::place, "d'agua"),
    "JSON_EXTRACT(payload, '$.tags.\"place\"') = 'd''agua'"
  );
}

#[test]
fn _08_runs_against_sqlite() {
  let conn = rusqlite::Connection::open_in_memory().expect("failed to open sqlite");
  conn
    .execute_batch(
      "CREATE TABLE t(payload BLOB);
       INSERT INTO t VALUES (jsonb('{\"tags\":{\"highway\":\"residential\",\"ISO3166-1\":\" ad \"}}'));",
    )
    .expect("failed to seed");

  let matched: i64 = conn
    .query_row(
      &format!(
        "SELECT COUNT(*) FROM t WHERE {} AND {}",
        equals("payload", osm_tag::highway, highway_value::residential.value()),
        is_null("payload", osm_tag::building)
      ),
      [],
      |row| row.get(0),
    )
    .expect("failed to query");
  assert_eq!(matched, 1);

  let iso: Option<String> = conn
    .query_row(
      &format!(
        "SELECT {} FROM t",
        normalized_coalesce("payload", key::COUNTRY_ISO)
      ),
      [],
      |row| row.get(0),
    )
    .expect("failed to query");
  assert_eq!(iso.as_deref(), Some("AD"));
}
