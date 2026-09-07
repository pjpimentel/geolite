use rusqlite::Connection;

use super::entity::admin_level;
use super::geometry::{admin_geometry, bounding_box};
use super::id::admin_level_id;
use super::scale::level;
use crate::domain::table;

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS admin_levels (
    id INTEGER PRIMARY KEY,
    relation_id INTEGER,
    way_id INTEGER,
    admin_level INTEGER NOT NULL,
    wkb BLOB NOT NULL,
    name VARCHAR(128) NOT NULL,
    -- country_iso_code: ISO 3166-1 alpha-2 (2 chars, e.g. 'BR', 'US')
    country_iso_code VARCHAR(3),
    post_code VARCHAR(12)
  );
";

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS admin_levels_search_by_level
    ON admin_levels (admin_level);
";

const SQL_DROP: &str = "DROP TABLE IF EXISTS admin_levels;";

const SQL_DROP_INDEXES: &str = "
  DROP INDEX IF EXISTS admin_levels_search_by_level;
";

pub struct admin_levels;

impl table for admin_levels {
  const CREATE: &'static str = SQL_CREATE;
  const INDEXES: &'static str = SQL_CREATE_INDEXES;
}

fn level_of(id: i64, raw: u8) -> Option<level> {
  let level = level::new(raw);
  if level.is_none() {
    eprintln!("warn: admin_levels row {id} carries level {raw}, outside the scale; skipped");
  }
  level
}

pub(crate) fn drop_table(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP)
    .expect("failed to drop admin_levels");
}

pub fn drop_indexes(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP_INDEXES)
    .expect("failed to drop admin_levels indexes");
}

pub struct admin_level_geom_row {
  pub id: i64,
  pub admin_level: level,
  pub name: String,
  pub wkb: Option<admin_geometry>,
  pub post_code: Option<String>,
}

fn map_geom_row(row: &rusqlite::Row) -> rusqlite::Result<Option<admin_level_geom_row>> {
  let id: i64 = row.get(0)?;
  let Some(admin_level) = level_of(id, row.get(1)?) else {
    return Ok(None);
  };
  Ok(Some(admin_level_geom_row {
    id,
    admin_level,
    name: row.get(2)?,
    wkb: row.get(3)?,
    post_code: row.get(4)?,
  }))
}

fn by_ids<T>(
  conn: &Connection,
  sql_prefix: &str,
  ids: &[i64],
  row_of: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
) -> Vec<T> {
  if ids.is_empty() {
    return vec![];
  }
  let sql = format!(
    "{} ({})",
    sql_prefix.trim(),
    crate::database::placeholders_for(ids.len())
  );
  crate::database::query_by_ids(conn, &sql, ids.iter().copied(), row_of)
}

pub fn load_all_below_street(conn: &Connection) -> Vec<admin_level_geom_row> {
  const SQL_LOAD_ALL_BELOW_STREET: &str = "
    SELECT
      id,
      admin_level,
      name,
      wkb,
      post_code
    FROM admin_levels
    WHERE admin_level < ?1
    ORDER BY admin_level ASC, wkb IS NULL ASC
  ";

  let mut stmt = conn
    .prepare(SQL_LOAD_ALL_BELOW_STREET)
    .expect("failed to prepare load ancestors");
  stmt
    .query_map([level::street.value()], map_geom_row)
    .expect("failed to query ancestors")
    .filter_map(|r| r.expect("failed to read ancestor row"))
    .collect()
}

pub struct street_query_row {
  pub id: i64,
  pub admin_level: level,
  pub wkb: Option<admin_geometry>,
}

pub fn streets_for_coordinates(
  conn: &Connection,
  lon: f64,
  lat: f64,
  delta: f64,
  bbox: bounding_box,
) -> Vec<street_query_row> {
  const SQL_STREETS_FOR_COORDINATES: &str = "
    SELECT
      al.id,
      al.admin_level,
      al.wkb
    FROM admin_levels al
    INNER JOIN admin_levels_rtree rt ON al.id = rt.id
    WHERE rt.min_lon <= ?1 AND rt.max_lon >= ?2
      AND rt.min_lat <= ?3 AND rt.max_lat >= ?4
      AND rt.min_lon <= ?5 AND rt.max_lon >= ?6
      AND rt.min_lat <= ?7 AND rt.max_lat >= ?8
      AND al.admin_level = ?9
  ";

  let mut stmt = conn
    .prepare(SQL_STREETS_FOR_COORDINATES)
    .expect("failed to prepare streets for coordinates");
  stmt
    .query_map(
      rusqlite::params![
        lon + delta,
        lon - delta,
        lat + delta,
        lat - delta,
        bbox.max_lon,
        bbox.min_lon,
        bbox.max_lat,
        bbox.min_lat,
        level::street.value()
      ],
      |row| {
        let id: i64 = row.get(0)?;
        Ok(level_of(id, row.get(1)?).map(|admin_level| street_query_row {
          id,
          admin_level,
          wkb: row.get(2).ok().flatten(),
        }))
      },
    )
    .expect("failed to query streets for coordinates")
    .filter_map(|r| r.expect("failed to read street row"))
    .collect()
}

pub fn ids_in_bounding_box(conn: &Connection, bbox: bounding_box) -> Vec<i64> {
  const SQL_IDS_IN_BOUNDING_BOX: &str = "
    SELECT id
    FROM admin_levels_rtree
    WHERE min_lon <= ?1 AND max_lon >= ?2
      AND min_lat <= ?3 AND max_lat >= ?4
  ";

  let mut stmt = conn
    .prepare(SQL_IDS_IN_BOUNDING_BOX)
    .expect("failed to prepare ids_in_bounding_box");
  stmt
    .query_map(
      rusqlite::params![bbox.max_lon, bbox.min_lon, bbox.max_lat, bbox.min_lat],
      |row| row.get::<_, i64>(0),
    )
    .expect("failed to query ids_in_bounding_box")
    .map(|r| r.expect("failed to read bounding box id"))
    .collect()
}

pub fn load_by_ids(conn: &Connection, ids: &[i64]) -> Vec<admin_level_geom_row> {
  const SQL_LOAD_BY_IDS: &str = "
    SELECT
      id,
      admin_level,
      name,
      wkb,
      post_code
    FROM admin_levels
    WHERE id IN
  ";

  by_ids(conn, SQL_LOAD_BY_IDS, ids, map_geom_row)
    .into_iter()
    .flatten()
    .collect()
}

pub struct admin_area_row {
  pub id: i64,
  pub name: String,
  pub admin_level: level,
  pub relation_id: Option<u64>,
  pub way_id: Option<u64>,
  pub wkb: Option<admin_geometry>,
}

pub fn load_full_by_ids(conn: &Connection, ids: &[i64]) -> Vec<admin_area_row> {
  const SQL_LOAD_FULL_BY_IDS: &str = "
    SELECT id, name, admin_level, relation_id, way_id, wkb
    FROM admin_levels
    WHERE id IN
  ";

  by_ids(conn, SQL_LOAD_FULL_BY_IDS, ids, |row| {
    let id: i64 = row.get(0)?;
    Ok(level_of(id, row.get(2)?).map(|admin_level| admin_area_row {
      id,
      name: row.get(1).unwrap_or_default(),
      admin_level,
      relation_id: row.get(3).ok().flatten(),
      way_id: row.get(4).ok().flatten(),
      wkb: row.get(5).ok().flatten(),
    }))
  })
  .into_iter()
  .flatten()
  .collect()
}

pub struct admin_meta_row {
  pub id: i64,
  pub name: String,
  pub admin_level: level,
  pub relation_id: Option<u64>,
  pub way_id: Option<u64>,
  pub country_iso_code: Option<String>,
  pub post_code: Option<String>,
}

pub fn load_metadata_by_ids(
  conn: &Connection,
  ids: &[i64],
) -> std::collections::HashMap<i64, admin_meta_row> {
  const SQL_LOAD_METADATA_BY_IDS: &str = "
    SELECT id, name, admin_level, relation_id, way_id, country_iso_code, post_code
    FROM admin_levels
    WHERE id IN
  ";

  by_ids(conn, SQL_LOAD_METADATA_BY_IDS, ids, |row| {
    let id: i64 = row.get(0)?;
    let Some(admin_level) = level_of(id, row.get(2)?) else {
      return Ok(None);
    };
    Ok(Some(admin_meta_row {
      id,
      name: row.get(1)?,
      admin_level,
      relation_id: row.get(3)?,
      way_id: row.get(4)?,
      country_iso_code: row.get(5)?,
      post_code: row.get(6)?,
    }))
  })
  .into_iter()
  .flatten()
  .map(|r| (r.id, r))
  .collect()
}

pub fn count_with_geometry(conn: &Connection) -> i64 {
  const SQL_COUNT_WITH_GEOMETRY: &str = "
    SELECT COUNT(*)
    FROM admin_levels
    WHERE wkb IS NOT NULL
  ";

  conn
    .query_row(SQL_COUNT_WITH_GEOMETRY, [], |row| row.get::<_, i64>(0))
    .expect("failed to count admin_levels with geometry")
}

pub fn id_range_with_geometry(conn: &Connection) -> (i64, i64) {
  const SQL_ID_RANGE_WITH_GEOMETRY: &str = "
    SELECT MIN(id), MAX(id)
    FROM admin_levels
    WHERE wkb IS NOT NULL
  ";

  conn
    .query_row(SQL_ID_RANGE_WITH_GEOMETRY, [], |row| {
      Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })
    .expect("failed to query id range with geometry")
}

pub fn load_wkb_page(
  conn: &Connection,
  last_id: i64,
  max_id: i64,
  limit: usize,
) -> Vec<(i64, admin_geometry)> {
  const SQL_LOAD_WKB_PAGE: &str = "
    SELECT id, wkb
    FROM admin_levels
    WHERE wkb IS NOT NULL
      AND id > ?1
      AND id <= ?2
    ORDER BY id ASC
    LIMIT ?3
  ";

  let mut stmt = conn
    .prepare_cached(SQL_LOAD_WKB_PAGE)
    .expect("failed to prepare load_wkb_page");
  stmt
    .query_map(rusqlite::params![last_id, max_id, limit as i64], |row| {
      Ok((row.get::<_, i64>(0)?, row.get::<_, admin_geometry>(1)?))
    })
    .expect("failed to query load_wkb_page")
    .map(|r| r.expect("failed to read wkb page row"))
    .collect()
}

pub fn batch_upsert(conn: &Connection, rows: &[admin_level]) -> i64 {
  const SQL_UPSERT: &str = "
    INSERT INTO admin_levels (
      id,
      relation_id,
      way_id,
      admin_level,
      name,
      country_iso_code,
      post_code,
      wkb
    ) VALUES (
      ?1,
      ?2,
      ?3,
      ?4,
      ?5,
      ?6,
      ?7,
      ?8
    )
    ON CONFLICT (id)
    DO UPDATE SET
      name             = excluded.name,
      country_iso_code = excluded.country_iso_code,
      post_code        = excluded.post_code,
      wkb              = excluded.wkb
  ";

  let tx = conn
    .unchecked_transaction()
    .expect("failed to begin transaction");
  let mut total_changes: i64 = 0;
  {
    let mut stmt = tx.prepare(SQL_UPSERT).expect("failed to prepare upsert");
    for row in rows {
      let id: u64 = match (row.relation_id, row.way_id) {
        (Some(rel), _) => admin_level_id::from_relation(rel).raw(),
        (None, Some(w)) => admin_level_id::from_way(w).raw(),
        (None, None) => panic!(
          "admin_levels row has neither way_id nor relation_id; \
           cannot derive a stable id (admin_level={}, name={:?})",
          row.level.value(),
          row.name,
        ),
      };
      let changes = stmt
        .execute(rusqlite::params![
          id,
          row.relation_id,
          row.way_id,
          row.level.value(),
          row.name,
          row.country_iso_code,
          row.post_code,
          row.wkb,
        ])
        .expect("failed to upsert admin_level row");
      total_changes += changes as i64;
    }
  }
  tx.commit().expect("failed to commit transaction");
  total_changes
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod tests;
