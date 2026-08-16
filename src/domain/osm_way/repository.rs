use rusqlite::Connection;

use super::entity::osm_way_row;
use super::filter::way_filter;
use crate::domain::admin_level::level;

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS osm_data.osm_ways (
    id INTEGER PRIMARY KEY,
    osm_pbf_chunk_id INTEGER,
    payload BLOB NOT NULL
  );
";

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS osm_data.osm_ways_search_by_chunk ON osm_ways(osm_pbf_chunk_id);
";

const SQL_DROP: &str = "DROP TABLE IF EXISTS osm_data.osm_ways;";

const INSERT_CHUNK_SIZE: usize = 10_000;

const SQL_INSERT_HEAD: &str = "
  INSERT OR IGNORE INTO osm_data.osm_ways (
    id,
    osm_pbf_chunk_id,
    payload
  ) VALUES
";

fn build_multi_insert_sql(n: usize) -> String {
  use std::fmt::Write;
  let mut sql = String::with_capacity(SQL_INSERT_HEAD.len() + n * 24);
  sql.push_str(SQL_INSERT_HEAD);
  for i in 0..n {
    if i > 0 {
      sql.push_str(",\n");
    }
    let base = i * 3;
    write!(sql, "  (?{}, ?{}, ?{})", base + 1, base + 2, base + 3).unwrap();
  }
  sql
}

pub(crate) fn create_table(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE)
    .expect("failed to create osm_ways");
}

#[allow(dead_code)]
pub(crate) fn drop_table(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP)
    .expect("failed to drop osm_ways");
}

pub fn create_indexes(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE_INDEXES)
    .expect("failed to create osm_ways indexes");
}

pub struct way_coord_row {
  pub way_id: u64,
  pub way_name: String,
  pub post_code: Option<String>,
  pub lon: f64,
  pub lat: f64,
}

const SQL_FILTER_PLACE_NEIGHBOURHOOD: &str =
  "json_extract(payload, '$.tags.place') = 'neighbourhood'";
const SQL_FILTER_PLACE_SUBURB: &str = "json_extract(payload, '$.tags.place') = 'suburb'";
const SQL_FILTER_HIGHWAY_RESIDENTIAL: &str =
  "json_extract(payload, '$.tags.highway') = 'residential'";
const SQL_FILTER_HIGHWAY_PRIMARY: &str = "json_extract(payload, '$.tags.highway') = 'primary'";
const SQL_FILTER_HIGHWAY_SECONDARY: &str = "json_extract(payload, '$.tags.highway') = 'secondary'";
const SQL_FILTER_HIGHWAY_TERTIARY: &str = "json_extract(payload, '$.tags.highway') = 'tertiary'";
const SQL_FILTER_HIGHWAY_UNCLASSIFIED: &str =
  "json_extract(payload, '$.tags.highway') = 'unclassified'";
const SQL_FILTER_HIGHWAY_LIVING_STREET: &str =
  "json_extract(payload, '$.tags.highway') = 'living_street'";
const SQL_FILTER_EXCLUDE_PLACE_NEIGHBOURHOOD: &str =
  "COALESCE(json_extract(payload, '$.tags.place'), '') NOT IN ('neighbourhood')";
const SQL_FILTER_EXCLUDE_PLACE_SUBURB: &str =
  "COALESCE(json_extract(payload, '$.tags.place'), '') NOT IN ('suburb')";
const SQL_FILTER_EXCLUDE_LEISURE_PARK: &str =
  "COALESCE(json_extract(payload, '$.tags.leisure'), '') NOT IN ('park')";
const SQL_FILTER_EXCLUDE_BUILDING: &str = "json_extract(payload, '$.tags.building') IS NULL";
const SQL_FILTER_EXCLUDE_WATERWAY: &str = "json_extract(payload, '$.tags.waterway') IS NULL";

// the predicate each filter stands for, over the way's json payload.
fn filter_sql(filter: &way_filter) -> &'static str {
  match filter {
    way_filter::include_place_neighbourhood => SQL_FILTER_PLACE_NEIGHBOURHOOD,
    way_filter::include_place_suburb => SQL_FILTER_PLACE_SUBURB,
    way_filter::include_highway_residential => SQL_FILTER_HIGHWAY_RESIDENTIAL,
    way_filter::include_highway_primary => SQL_FILTER_HIGHWAY_PRIMARY,
    way_filter::include_highway_secondary => SQL_FILTER_HIGHWAY_SECONDARY,
    way_filter::include_highway_tertiary => SQL_FILTER_HIGHWAY_TERTIARY,
    way_filter::include_highway_unclassified => SQL_FILTER_HIGHWAY_UNCLASSIFIED,
    way_filter::include_highway_living_street => SQL_FILTER_HIGHWAY_LIVING_STREET,
    way_filter::exclude_place_neighbourhood => SQL_FILTER_EXCLUDE_PLACE_NEIGHBOURHOOD,
    way_filter::exclude_place_suburb => SQL_FILTER_EXCLUDE_PLACE_SUBURB,
    way_filter::exclude_leisure_park => SQL_FILTER_EXCLUDE_LEISURE_PARK,
    way_filter::exclude_building => SQL_FILTER_EXCLUDE_BUILDING,
    way_filter::exclude_waterway => SQL_FILTER_EXCLUDE_WATERWAY,
  }
}

pub fn remaining_ids_by_tags(conn: &Connection, level: level, filter: &[way_filter]) -> Vec<u64> {
  let level = level.value();
  let filter_clauses: Vec<&str> = filter.iter().map(filter_sql).collect();
  let filter_part = if filter_clauses.is_empty() {
    String::new()
  } else {
    format!("\n    AND {}", filter_clauses.join("\n    AND "))
  };
  let sql = format!(
    "
    SELECT osm_data.osm_ways.id
    FROM osm_data.osm_ways
    LEFT JOIN main.admin_levels AS already_indexed
      ON already_indexed.way_id = osm_data.osm_ways.id
      AND already_indexed.admin_level = {level}
    WHERE json_extract(osm_data.osm_ways.payload, '$.tags.name') IS NOT NULL
      AND already_indexed.way_id IS NULL
      {filter_part}
    "
  );
  let mut stmt = conn
    .prepare(&sql)
    .expect("failed to prepare way candidates by tags");
  stmt
    .query_map([], |row| row.get::<_, u64>(0))
    .expect("failed to query way candidates by tags")
    .map(|r| r.expect("failed to read way candidate id"))
    .collect()
}

pub fn way_coords_chunk(
  conn: &Connection,
  ids: &[u64],
  name_priority: &[&str],
) -> Vec<way_coord_row> {
  let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
  let name_select = crate::database::name_select::build_name_select("osm_data.osm_ways.payload", name_priority);
  let sql = format!(
    "SELECT
           osm_data.osm_ways.id AS way_id,
           {name_select} AS way_name,
           NULLIF(UPPER(TRIM(COALESCE(
             JSON_EXTRACT(osm_data.osm_ways.payload, '$.tags.postal_code'),
             JSON_EXTRACT(osm_data.osm_ways.payload, '$.tags.\"addr:postcode\"')
           ))), '') AS post_code,
           CAST(JSON_EXTRACT(osm_data.osm_nodes.payload, '$.lon') AS REAL) AS lon,
           CAST(JSON_EXTRACT(osm_data.osm_nodes.payload, '$.lat') AS REAL) AS lat
         FROM osm_data.osm_ways,
              JSON_EACH(JSON_EXTRACT(osm_data.osm_ways.payload, '$.refs')) AS node_refs
         INNER JOIN osm_data.osm_nodes ON osm_data.osm_nodes.id = CAST(node_refs.value AS INTEGER)
         WHERE osm_data.osm_ways.id IN ({placeholders})
         ORDER BY osm_data.osm_ways.id ASC, node_refs.key ASC",
  );
  let params: Vec<rusqlite::types::Value> = ids
    .iter()
    .map(|&id| rusqlite::types::Value::Integer(id as i64))
    .collect();
  let mut stmt = conn
    .prepare(&sql)
    .expect("failed to prepare way coords query");
  stmt
    .query_map(rusqlite::params_from_iter(params.iter()), |row| {
      Ok(way_coord_row {
        way_id: row.get(0)?,
        way_name: row.get(1)?,
        post_code: row.get(2)?,
        lon: row.get(3)?,
        lat: row.get(4)?,
      })
    })
    .expect("failed to query way coords")
    .map(|r| r.expect("failed to read way coord row"))
    .collect()
}

pub fn insert_rows(conn: &Connection, rows: &[osm_way_row]) {
  if rows.is_empty() {
    return;
  }
  for chunk in rows.chunks(INSERT_CHUNK_SIZE) {
    let sql = build_multi_insert_sql(chunk.len());
    let mut stmt = conn
      .prepare_cached(&sql)
      .expect("failed to prepare osm_ways insert");
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(chunk.len() * 3);
    for row in chunk {
      params.push(&row.id);
      params.push(&row.osm_pbf_chunk_id);
      params.push(&row.payload);
    }
    stmt
      .execute(rusqlite::params_from_iter(params))
      .expect("failed to insert osm_way");
  }
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod tests;
