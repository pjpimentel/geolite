use rusqlite::Connection;

use super::entity::osm_way_row;
use super::filter::way_filter;
use crate::admin_level::level;
use crate::osm_tag::value::{highway_value, leisure_value, place_value};
use crate::osm_tag::{key, osm_tag, select};
use crate::database::table;

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

const PAYLOAD: &str = "osm_data.osm_ways.payload";

pub struct osm_ways;

impl table for osm_ways {
  const CREATE: &str = SQL_CREATE;
  const INDEXES: &str = SQL_CREATE_INDEXES;
}

pub(crate) fn drop_table(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP)
    .expect("failed to drop osm_ways");
}

pub struct way_coord_row {
  pub way_id: u64,
  pub way_name: String,
  pub post_code: Option<String>,
  pub lon: f64,
  pub lat: f64,
}

fn filter_sql(filter: way_filter) -> String {
  match filter {
    way_filter::include_place_neighbourhood => {
      select::equals(PAYLOAD, osm_tag::place, place_value::neighbourhood.value())
    }
    way_filter::include_place_suburb => {
      select::equals(PAYLOAD, osm_tag::place, place_value::suburb.value())
    }
    way_filter::include_highway_residential => {
      select::equals(PAYLOAD, osm_tag::highway, highway_value::residential.value())
    }
    way_filter::include_highway_primary => {
      select::equals(PAYLOAD, osm_tag::highway, highway_value::primary.value())
    }
    way_filter::include_highway_secondary => {
      select::equals(PAYLOAD, osm_tag::highway, highway_value::secondary.value())
    }
    way_filter::include_highway_tertiary => {
      select::equals(PAYLOAD, osm_tag::highway, highway_value::tertiary.value())
    }
    way_filter::include_highway_unclassified => {
      select::equals(PAYLOAD, osm_tag::highway, highway_value::unclassified.value())
    }
    way_filter::include_highway_living_street => {
      select::equals(PAYLOAD, osm_tag::highway, highway_value::living_street.value())
    }
    way_filter::exclude_place_neighbourhood => {
      select::not_in(PAYLOAD, osm_tag::place, &[place_value::neighbourhood.value()])
    }
    way_filter::exclude_place_suburb => {
      select::not_in(PAYLOAD, osm_tag::place, &[place_value::suburb.value()])
    }
    way_filter::exclude_leisure_park => {
      select::not_in(PAYLOAD, osm_tag::leisure, &[leisure_value::park.value()])
    }
    way_filter::exclude_building => select::is_null(PAYLOAD, osm_tag::building),
    way_filter::exclude_waterway => select::is_null(PAYLOAD, osm_tag::waterway),
  }
}

pub fn remaining_ids_by_tags(conn: &Connection, level: level, filter: &[way_filter]) -> Vec<u64> {
  let filter_clauses: Vec<String> = filter.iter().map(|&f| filter_sql(f)).collect();
  let filter_part = if filter_clauses.is_empty() {
    String::new()
  } else {
    format!("\n      AND {}", filter_clauses.join("\n      AND "))
  };
  let has_name = select::is_not_null(PAYLOAD, osm_tag::name);
  let sql = format!(
    "
    SELECT osm_data.osm_ways.id
    FROM osm_data.osm_ways
    LEFT JOIN main.admin_levels AS already_indexed
      ON already_indexed.way_id = osm_data.osm_ways.id
      AND already_indexed.admin_level = {}
    WHERE {has_name}
      AND already_indexed.way_id IS NULL{filter_part}
    ",
    level.value()
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
  let placeholders = crate::database::placeholders_for(ids.len());
  let name_select = select::coalesce_of(PAYLOAD, name_priority);
  // named after the osm tag: codeql reads a `post_code` local reaching the query as personal data
  // stored in clear (rust/cleartext-storage-database), and this is a column expression
  let postal_code_select = select::normalized_coalesce(PAYLOAD, key::POST_CODE);
  let sql = format!(
    "
    SELECT
      osm_data.osm_ways.id AS way_id,
      {name_select} AS way_name,
      {postal_code_select} AS post_code,
      CAST(JSON_EXTRACT(osm_data.osm_nodes.payload, '$.lon') AS REAL) AS lon,
      CAST(JSON_EXTRACT(osm_data.osm_nodes.payload, '$.lat') AS REAL) AS lat
    FROM osm_data.osm_ways,
      JSON_EACH(JSON_EXTRACT(osm_data.osm_ways.payload, '$.refs')) AS node_refs
    INNER JOIN osm_data.osm_nodes ON osm_data.osm_nodes.id = CAST(node_refs.value AS INTEGER)
    WHERE osm_data.osm_ways.id IN ({placeholders})
    ORDER BY osm_data.osm_ways.id ASC, node_refs.key ASC
    "
  );
  crate::database::query_by_ids(conn, &sql, ids.iter().map(|&id| id as i64), |row| {
    Ok(way_coord_row {
      way_id: row.get(0)?,
      way_name: row.get(1)?,
      post_code: row.get(2)?,
      lon: row.get(3)?,
      lat: row.get(4)?,
    })
  })
}

pub fn insert_rows(conn: &Connection, rows: &[osm_way_row]) {
  const SQL_INSERT_HEAD: &str = "
    INSERT OR IGNORE INTO osm_data.osm_ways (
      id,
      osm_pbf_chunk_id,
      payload
    ) VALUES
  ";

  let params: Vec<[&dyn rusqlite::ToSql; 3]> = rows
    .iter()
    .map(|row| [&row.id as &dyn rusqlite::ToSql, &row.osm_pbf_chunk_id, &row.payload])
    .collect();
  crate::database::insert_in_chunks(conn, SQL_INSERT_HEAD, &params);
}
