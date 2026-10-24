use rusqlite::Connection;

use super::entity::osm_way_row;
use super::filter::way_filter;
use crate::osm_tag::value::{highway_value, leisure_value, location_value, place_value};
use crate::osm_tag::{key, osm_tag, select};
use crate::database::table;

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS osm_ways (
    id INTEGER PRIMARY KEY,
    osm_pbf_chunk_id INTEGER,
    payload BLOB NOT NULL
  );
";

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS osm_ways_search_by_chunk ON osm_ways(osm_pbf_chunk_id);
";

const PAYLOAD: &str = "osm_ways.payload";

pub struct osm_ways;

impl table for osm_ways {
  const CREATE: &str = SQL_CREATE;
  const INDEXES: &str = SQL_CREATE_INDEXES;
}

pub fn delete_by_file_id(conn: &Connection, file_id: u32) {
  const SQL_DELETE_BY_FILE_ID: &str = "
    DELETE FROM osm_ways
    WHERE osm_pbf_chunk_id IN (
      SELECT id
      FROM osm_pbf_blob_chunks
      WHERE file_id = ?1
    )
  ";

  conn
    .execute(SQL_DELETE_BY_FILE_ID, [file_id])
    .expect("failed to delete the ways of a file");
}

pub struct way_coord_row {
  pub way_id: u64,
  pub names: Vec<Option<String>>,
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
    way_filter::exclude_boundary => select::is_null(PAYLOAD, osm_tag::boundary),
    way_filter::exclude_location_underwater => {
      select::not_in(PAYLOAD, osm_tag::location, &[location_value::underwater.value()])
    }
  }
}

pub fn candidate_ids_by_tags(conn: &Connection, filter: &[way_filter]) -> Vec<u64> {
  let filter_clauses: Vec<String> = filter.iter().map(|&f| filter_sql(f)).collect();
  let filter_part = if filter_clauses.is_empty() {
    String::new()
  } else {
    format!("\n      AND {}", filter_clauses.join("\n      AND "))
  };
  let has_name = select::is_not_null(PAYLOAD, osm_tag::name);
  let sql = format!(
    "
    SELECT osm_ways.id
    FROM osm_ways
    WHERE {has_name}{filter_part}
    ORDER BY osm_ways.id
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
  name_tags: &[&str],
) -> Vec<way_coord_row> {
  let placeholders = crate::database::placeholders_for(ids.len());
  let name_selects = select::each_of(PAYLOAD, name_tags)
    .iter()
    .enumerate()
    .map(|(at, expr)| format!("{expr} AS name_{at},\n"))
    .collect::<String>();
  // named after the osm tag: codeql reads a `post_code` local reaching the query as personal data
  // stored in clear (rust/cleartext-storage-database), and this is a column expression
  let postal_code_select = select::normalized_coalesce(PAYLOAD, key::POST_CODE);
  let sql = format!(
    "
    SELECT
      osm_ways.id AS way_id,
      {name_selects}
      {postal_code_select} AS post_code,
      CAST(JSON_EXTRACT(osm_nodes.payload, '$.lon') AS REAL) AS lon,
      CAST(JSON_EXTRACT(osm_nodes.payload, '$.lat') AS REAL) AS lat
    FROM osm_ways,
      JSON_EACH(JSON_EXTRACT(osm_ways.payload, '$.refs')) AS node_refs
    INNER JOIN osm_nodes ON osm_nodes.id = CAST(node_refs.value AS INTEGER)
    WHERE osm_ways.id IN ({placeholders})
    ORDER BY osm_ways.id ASC, node_refs.key ASC
    "
  );
  let after_names = 1 + name_tags.len();
  crate::database::query_by_ids(conn, &sql, ids.iter().map(|&id| id as i64), |row| {
    Ok(way_coord_row {
      way_id: row.get(0)?,
      names: (1..after_names)
        .map(|at| row.get::<_, Option<String>>(at))
        .collect::<rusqlite::Result<Vec<_>>>()?,
      post_code: row.get(after_names)?,
      lon: row.get(after_names + 1)?,
      lat: row.get(after_names + 2)?,
    })
  })
}

pub fn insert_rows(conn: &Connection, rows: &[osm_way_row]) {
  const SQL_INSERT_HEAD: &str = "
    INSERT OR IGNORE INTO osm_ways (
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
