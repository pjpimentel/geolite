use geo::Geometry;
use rusqlite::Connection;

use super::entity::house_number_link;
use super::policy::house_number_policy;
use super::value::house_number;
use crate::domain::admin_level::geometry::{admin_geometry, mbr_center};
use crate::domain::admin_level::level;
use crate::domain::table;

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS house_numbers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    node_id INTEGER NOT NULL UNIQUE,
    admin_level_id INTEGER NOT NULL REFERENCES admin_levels(id) ON DELETE CASCADE,
    number VARCHAR(12) NOT NULL,
    wkb BLOB,
    strategy INTEGER NOT NULL
  );
";

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS house_numbers_search_by_admin_level_id ON house_numbers(admin_level_id);
";

const SQL_DROP: &str = "DROP TABLE IF EXISTS house_numbers;";

const SQL_DROP_INDEXES: &str = "
  DROP INDEX IF EXISTS house_numbers_search_by_admin_level_id;
";

pub struct house_numbers;

impl table for house_numbers {
  const CREATE: &str = SQL_CREATE;
  const INDEXES: &str = SQL_CREATE_INDEXES;
}

pub(crate) fn drop_table(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP)
    .expect("failed to drop house_numbers");
}

pub fn drop_indexes(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP_INDEXES)
    .expect("failed to drop house_numbers indexes");
}

pub struct street_meta_row {
  pub id: i64,
  pub name: String,
  pub cx: f64,
  pub cy: f64,
}

pub fn streets_with_centroid(conn: &Connection) -> Vec<street_meta_row> {
  const SQL_STREETS_WITH_GEOMETRY: &str = "
    SELECT id, name, wkb
    FROM admin_levels
    WHERE admin_level = ?1
      AND wkb IS NOT NULL
  ";

  let mut stmt = conn
    .prepare(SQL_STREETS_WITH_GEOMETRY)
    .expect("failed to prepare streets with geometry");
  stmt
    .query_map([level::street.value()], |row| {
      Ok((
        row.get::<_, i64>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, Vec<u8>>(2)?,
      ))
    })
    .expect("failed to query streets")
    .filter_map(|r| {
      let (id, name, blob) = r.expect("failed to read street meta row");
      let (cx, cy) = mbr_center(&blob)?;
      Some(street_meta_row { id, name, cx, cy })
    })
    .collect()
}

pub struct street_wkb_row {
  pub id: i64,
  pub wkb: admin_geometry,
}

pub fn streets_wkb_by_ids(conn: &Connection, ids: &[i64]) -> Vec<street_wkb_row> {
  const SQL_STREETS_WKB_BY_IDS: &str = "
    SELECT id, wkb
    FROM admin_levels
    WHERE wkb IS NOT NULL
      AND id IN
  ";

  if ids.is_empty() {
    return vec![];
  }
  let sql = format!(
    "{} ({})",
    SQL_STREETS_WKB_BY_IDS.trim(),
    crate::database::placeholders_for(ids.len())
  );
  crate::database::query_by_ids(conn, &sql, ids.iter().copied(), |row| {
    Ok(street_wkb_row {
      id: row.get(0)?,
      wkb: row.get(1)?,
    })
  })
}

pub struct candidate_row {
  pub id: u64,
  pub number: house_number,
  pub addr_street: Option<String>,
  pub lon: f64,
  pub lat: f64,
}

pub fn load_all_candidates(conn: &Connection, policy: &house_number_policy) -> Vec<candidate_row> {
  const SQL_LOAD_ALL_CANDIDATES: &str = "
    SELECT
      id,
      CAST({number_select} AS TEXT) AS number,
      {street_select} AS addr_street,
      CAST(payload->>'lon' AS REAL) AS lon,
      CAST(payload->>'lat' AS REAL) AS lat
    FROM osm_data.osm_nodes
    WHERE {number_select} IS NOT NULL
  ";

  debug_assert!(
    !policy.number_tags.is_empty(),
    "number_tags must not be empty"
  );
  let number_select = crate::domain::osm_tag::select::coalesce_of("payload", policy.number_tags);
  let street_select = crate::domain::osm_tag::select::coalesce_of("payload", policy.street_tags);
  let sql = SQL_LOAD_ALL_CANDIDATES
    .replace("{number_select}", &number_select)
    .replace("{street_select}", &street_select);
  let mut stmt = conn
    .prepare(&sql)
    .expect("failed to prepare load all candidates");
  stmt
    .query_map([], |row| {
      Ok((
        row.get::<_, u64>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, Option<String>>(2)?,
        row.get::<_, f64>(3)?,
        row.get::<_, f64>(4)?,
      ))
    })
    .expect("failed to query candidates")
    .map(|r| r.expect("failed to read candidate row"))
    .filter_map(|(id, raw, addr_street, lon, lat)| {
      house_number::normalize(&raw, policy).map(|number| candidate_row {
        id,
        number,
        addr_street,
        lon,
        lat,
      })
    })
    .collect()
}

pub struct hn_for_street {
  pub admin_level_id: i64,
  pub number: house_number,
  pub wkb: Option<admin_geometry>,
}

pub fn by_admin_level_ids(conn: &Connection, ids: &[i64]) -> Vec<hn_for_street> {
  const SQL_BY_ADMIN_LEVEL_IDS: &str = "
    SELECT admin_level_id, number, wkb
    FROM house_numbers
    WHERE admin_level_id IN
  ";

  if ids.is_empty() {
    return vec![];
  }
  let sql = format!(
    "{} ({})",
    SQL_BY_ADMIN_LEVEL_IDS.trim(),
    crate::database::placeholders_for(ids.len())
  );
  crate::database::query_by_ids(conn, &sql, ids.iter().copied(), |row| {
    Ok(hn_for_street {
      admin_level_id: row.get(0)?,
      number: house_number::from_stored(&row.get::<_, String>(1)?),
      wkb: row.get(2)?,
    })
  })
}

pub fn batch_insert_links(conn: &Connection, links: &[house_number_link]) -> i64 {
  const SQL_INSERT: &str = "
    INSERT OR IGNORE INTO house_numbers (
      node_id,
      admin_level_id,
      number,
      wkb,
      strategy
    ) VALUES (
      ?1,
      ?2,
      ?3,
      ?4,
      ?5
    );
  ";

  if links.is_empty() {
    return 0;
  }
  let tx = conn
    .unchecked_transaction()
    .expect("failed to begin transaction");
  let mut total: i64 = 0;
  {
    let mut stmt = tx
      .prepare(SQL_INSERT)
      .expect("failed to prepare house_numbers insert");
    for link in links {
      let wkb: admin_geometry = Geometry::Point(link.point).into();
      let changes = stmt
        .execute(rusqlite::params![
          link.node_id,
          link.street_id.raw() as i64,
          link.number.stored_form(),
          wkb,
          link.strategy.code(),
        ])
        .expect("failed to insert house_number");
      total += changes as i64;
    }
  }
  tx.commit().expect("failed to commit");
  total
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod tests;
