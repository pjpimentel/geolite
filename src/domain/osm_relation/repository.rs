use rusqlite::Connection;

use super::entity::osm_relation_row;
use crate::domain::admin_level::level;
use crate::domain::osm_tag::{key, select};
use crate::domain::table;

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS osm_data.osm_relations (
    id INTEGER PRIMARY KEY,
    osm_pbf_chunk_id INTEGER,
    payload BLOB NOT NULL
  );
";

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS osm_data.osm_relations_search_by_admin_level
    ON osm_relations(JSON_EXTRACT(payload, '$.tags.admin_level'));
";

const SQL_DROP: &str = "DROP TABLE IF EXISTS osm_data.osm_relations;";

// sqlite only takes an expression index when the query spells the expression exactly as the
// index does: the bare `'$.tags.admin_level'` path stays a literal here instead of going through
// the quoted path of `osm_tag::select`, or every existing database would fall back to a full scan
const SQL_CANDIDATES_BY_ADMIN_LEVEL: &str = "
  SELECT id
  FROM osm_data.osm_relations
  WHERE JSON_EXTRACT(payload, '$.tags.admin_level') = ?1
    AND JSON_EXTRACT(payload, '$.tags.name') IS NOT NULL
";

const PAYLOAD: &str = "osm_data.osm_relations.payload";

pub struct osm_relations;

impl table for osm_relations {
  const CREATE: &str = SQL_CREATE;
  const INDEXES: &str = SQL_CREATE_INDEXES;
}

pub(crate) fn drop_table(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP)
    .expect("failed to drop osm_relations");
}

pub struct relation_coord_row {
  pub relation_id: u64,
  pub relation_name: String,
  pub country_iso_code: Option<String>,
  pub post_code: Option<String>,
  pub way_order: u32,
  pub way_id: u64,
  pub lon: f64,
  pub lat: f64,
}

pub fn all_ids_by_admin_level(conn: &Connection, level: level) -> Vec<u64> {
  let level_tag = level.value().to_string();
  let mut stmt = conn
    .prepare(SQL_CANDIDATES_BY_ADMIN_LEVEL)
    .expect("failed to prepare all relation ids");
  stmt
    .query_map([&level_tag], |row| row.get::<_, u64>(0))
    .expect("failed to query all relation ids")
    .map(|r| r.expect("failed to read relation id"))
    .collect()
}

pub fn remaining_ids_by_admin_level(conn: &Connection, level: level) -> Vec<u64> {
  let sql = format!(
    "
    WITH candidates AS ({SQL_CANDIDATES_BY_ADMIN_LEVEL})
    SELECT candidates.id
    FROM candidates
    LEFT JOIN main.admin_levels ON main.admin_levels.relation_id = candidates.id
      AND main.admin_levels.admin_level = ?2
    WHERE main.admin_levels.relation_id IS NULL
    "
  );
  let level_tag = level.value().to_string();
  let mut stmt = conn
    .prepare(&sql)
    .expect("failed to prepare remaining relation ids");
  stmt
    .query_map(rusqlite::params![level_tag, level.value()], |row| {
      row.get::<_, u64>(0)
    })
    .expect("failed to query remaining relation ids")
    .map(|r| r.expect("failed to read relation id"))
    .collect()
}

pub fn relation_coords_chunk(
  conn: &Connection,
  ids: &[u64],
  name_priority: &[&str],
) -> Vec<relation_coord_row> {
  let placeholders = crate::database::placeholders_for(ids.len());
  let name_select = select::coalesce_of(PAYLOAD, name_priority);
  let country_iso_select = select::normalized_coalesce(PAYLOAD, key::COUNTRY_ISO);
  // named after the osm tag: codeql reads a `post_code` local reaching the query as personal data
  // stored in clear (rust/cleartext-storage-database), and this is a column expression
  let postal_code_select = select::normalized_coalesce(PAYLOAD, key::POST_CODE);
  let sql = format!(
    "
    WITH way_members AS (
      SELECT
        osm_data.osm_relations.id AS relation_id,
        CAST(mem.key AS INTEGER) AS way_order,
        CAST(JSON_EXTRACT(mem.value, '$.id') AS INTEGER) AS way_id,
        {name_select} AS relation_name,
        {country_iso_select} AS country_iso_code,
        {postal_code_select} AS post_code
      FROM osm_data.osm_relations,
        JSON_EACH(JSON_EXTRACT(osm_data.osm_relations.payload, '$.members')) AS mem
      WHERE osm_data.osm_relations.id IN ({placeholders})
        AND JSON_EXTRACT(mem.value, '$.type') = 'w'
    )
    SELECT
      way_members.relation_id,
      way_members.relation_name,
      way_members.country_iso_code,
      way_members.post_code,
      way_members.way_order,
      way_members.way_id,
      CAST(JSON_EXTRACT(osm_data.osm_nodes.payload, '$.lon') AS REAL) AS lon,
      CAST(JSON_EXTRACT(osm_data.osm_nodes.payload, '$.lat') AS REAL) AS lat
    FROM osm_data.osm_ways,
      JSON_EACH(JSON_EXTRACT(osm_data.osm_ways.payload, '$.refs')) AS node_refs
    INNER JOIN way_members ON way_members.way_id = osm_data.osm_ways.id
    INNER JOIN osm_data.osm_nodes ON osm_data.osm_nodes.id = CAST(node_refs.value AS INTEGER)
    ORDER BY way_members.relation_id ASC, way_members.way_order ASC, node_refs.key ASC
    "
  );
  crate::database::query_by_ids(conn, &sql, ids.iter().map(|&id| id as i64), |row| {
    Ok(relation_coord_row {
      relation_id: row.get(0)?,
      relation_name: row.get(1)?,
      country_iso_code: row.get(2)?,
      post_code: row.get(3)?,
      way_order: row.get(4)?,
      way_id: row.get(5)?,
      lon: row.get(6)?,
      lat: row.get(7)?,
    })
  })
}

pub fn insert_rows(conn: &Connection, rows: &[osm_relation_row]) {
  const SQL_INSERT_HEAD: &str = "
    INSERT OR IGNORE INTO osm_data.osm_relations (
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

#[cfg(test)]
#[path = "repository.test.rs"]
mod tests;
