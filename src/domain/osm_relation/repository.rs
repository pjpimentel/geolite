// the `osm_data.osm_relations` table: the ddl, the admin-level index, the bulk insert, and the
// queries that find the relations a level wants and stitch their member ways back into coordinates.

use rusqlite::Connection;

use super::entity::osm_relation_row;
use crate::domain::admin_level::level;
use crate::domain::osm_tag::{key, select};

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS osm_data.osm_relations (
    id INTEGER PRIMARY KEY,
    osm_pbf_chunk_id INTEGER,
    payload BLOB NOT NULL
  );
";

const SQL_DROP: &str = "DROP TABLE IF EXISTS osm_data.osm_relations;";

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS osm_data.osm_relations_search_by_admin_level
    ON osm_relations(JSON_EXTRACT(payload, '$.tags.admin_level'));
";

pub(crate) fn create_table(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE)
    .expect("failed to create osm_relations");
}

#[allow(dead_code)]
pub(crate) fn drop_table(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP)
    .expect("failed to drop osm_relations");
}

pub fn create_indexes(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE_INDEXES)
    .expect("failed to create osm_relations indexes");
}

const SQL_ALL_IDS_BY_ADMIN_LEVEL: &str = "
  SELECT id
  FROM osm_data.osm_relations
  WHERE JSON_EXTRACT(payload, '$.tags.admin_level') = ?1
  AND JSON_EXTRACT(payload, '$.tags.name') IS NOT NULL
";

pub fn all_ids_by_admin_level(conn: &Connection, level: level) -> Vec<u64> {
  let level_str = level.value().to_string();
  let mut stmt = conn
    .prepare(SQL_ALL_IDS_BY_ADMIN_LEVEL)
    .expect("failed to prepare all relation ids");
  stmt
    .query_map([&level_str], |row| row.get::<_, u64>(0))
    .expect("failed to query all relation ids")
    .map(|r| r.expect("failed to read relation id"))
    .collect()
}

const SQL_REMAINING_IDS_BY_ADMIN_LEVEL: &str = "
  WITH candidates AS (
    SELECT id
    FROM osm_data.osm_relations
    WHERE JSON_EXTRACT(payload, '$.tags.admin_level') = ?1
    AND JSON_EXTRACT(payload, '$.tags.name') IS NOT NULL
  )
  SELECT candidates.id
  FROM candidates
  LEFT JOIN main.admin_levels ON main.admin_levels.relation_id = candidates.id
    AND main.admin_levels.admin_level = ?2
  WHERE main.admin_levels.relation_id IS NULL
";

pub fn remaining_ids_by_admin_level(conn: &Connection, level: level) -> Vec<u64> {
  let level_str = level.value().to_string();
  let mut stmt = conn
    .prepare(SQL_REMAINING_IDS_BY_ADMIN_LEVEL)
    .expect("failed to prepare remaining relation ids");
  stmt
    .query_map(rusqlite::params![level_str, level.value()], |row| {
      row.get::<_, u64>(0)
    })
    .expect("failed to query remaining relation ids")
    .map(|r| r.expect("failed to read relation id"))
    .collect()
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

pub fn relation_coords_chunk(
  conn: &Connection,
  ids: &[u64],
  name_priority: &[&str],
) -> Vec<relation_coord_row> {
  let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
  let payload = "osm_data.osm_relations.payload";
  let name_select = select::coalesce_of(payload, name_priority);
  let country_iso_select = select::normalized_coalesce(payload, key::COUNTRY_ISO);
  let post_code_select = select::normalized_coalesce(payload, key::POST_CODE);
  let sql = format!(
    "WITH way_members AS (
       SELECT
         osm_data.osm_relations.id AS relation_id,
         CAST(mem.key AS INTEGER) AS way_order,
         CAST(JSON_EXTRACT(mem.value, '$.id') AS INTEGER) AS way_id,
         {name_select} AS relation_name,
         {country_iso_select} AS country_iso_code,
         {post_code_select} AS post_code
       FROM osm_data.osm_relations, JSON_EACH(JSON_EXTRACT(osm_data.osm_relations.payload, '$.members')) AS mem
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
     ORDER BY way_members.relation_id ASC, way_members.way_order ASC, node_refs.key ASC",
  );
  let params: Vec<rusqlite::types::Value> = ids
    .iter()
    .map(|&id| rusqlite::types::Value::Integer(id as i64))
    .collect();
  let mut stmt = conn
    .prepare(&sql)
    .expect("failed to prepare relation coords query");
  stmt
    .query_map(rusqlite::params_from_iter(params.iter()), |row| {
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
    .expect("failed to query relation coords")
    .map(|r| r.expect("failed to read relation coord row"))
    .collect()
}

const INSERT_CHUNK_SIZE: usize = 10_000;

const SQL_INSERT_HEAD: &str = "
  INSERT OR IGNORE INTO osm_data.osm_relations (
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

pub fn insert_rows(conn: &Connection, rows: &[osm_relation_row]) {
  if rows.is_empty() {
    return;
  }
  for chunk in rows.chunks(INSERT_CHUNK_SIZE) {
    let sql = build_multi_insert_sql(chunk.len());
    let mut stmt = conn
      .prepare_cached(&sql)
      .expect("failed to prepare osm_relations insert");
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(chunk.len() * 3);
    for row in chunk {
      params.push(&row.id);
      params.push(&row.osm_pbf_chunk_id);
      params.push(&row.payload);
    }
    stmt
      .execute(rusqlite::params_from_iter(params))
      .expect("failed to insert osm_relation");
  }
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod tests;
