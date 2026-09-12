use rusqlite::Connection;
use std::collections::HashMap;

use super::entity::{decode_chain, hierarchy_lookup_row, hierarchy_row};
use crate::domain::admin_level::level;
use crate::domain::table;

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS admin_levels_hierarchy (
    admin_level_id INTEGER PRIMARY KEY REFERENCES admin_levels(id) ON DELETE CASCADE,
    ancestor_ids BLOB NOT NULL,
    user_friendly_name TEXT NOT NULL
  );
";

const SQL_DROP: &str = "DROP TABLE IF EXISTS admin_levels_hierarchy;";

pub struct admin_levels_hierarchy;

impl table for admin_levels_hierarchy {
  const CREATE: &str = SQL_CREATE;
  const INDEXES: &str = "";
}

pub(crate) fn drop_table(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP)
    .expect("failed to drop admin_levels_hierarchy");
}

pub fn destroy(conn: &Connection) {
  drop_table(conn);
  admin_levels_hierarchy::create_table(conn);
}

pub fn count(conn: &Connection) -> i64 {
  const SQL_COUNT: &str = "
    SELECT COUNT(*) FROM admin_levels_hierarchy
  ";

  conn
    .query_row(SQL_COUNT, [], |row| row.get(0))
    .expect("failed to count admin_levels_hierarchy")
}

pub fn pending_total(conn: &Connection) -> i64 {
  const SQL_PENDING_TOTAL: &str = "
    WITH pending AS (
      SELECT al.id
      FROM admin_levels al
      LEFT JOIN admin_levels_hierarchy h ON al.id = h.admin_level_id
      WHERE h.admin_level_id IS NULL
    )
    SELECT COUNT(*) FROM pending
  ";

  conn
    .query_row(SQL_PENDING_TOTAL, [], |row| row.get::<_, i64>(0))
    .expect("failed to query pending total")
}

pub fn pending_street_ids(conn: &Connection) -> Vec<i64> {
  const SQL_PENDING_STREET_IDS: &str = "
    WITH already_indexed AS (
      SELECT admin_level_id FROM admin_levels_hierarchy
    )
    SELECT al.id
    FROM admin_levels al
    LEFT JOIN already_indexed ai ON al.id = ai.admin_level_id
    WHERE al.admin_level = ?1
      AND ai.admin_level_id IS NULL
    ORDER BY al.id ASC
  ";

  let mut stmt = conn
    .prepare(SQL_PENDING_STREET_IDS)
    .expect("failed to prepare pending streets");
  stmt
    .query_map([level::street.value()], |row| row.get::<_, i64>(0))
    .expect("failed to query pending streets")
    .map(|r| r.expect("failed to read street id"))
    .collect()
}

pub fn load_by_ids(conn: &Connection, ids: &[i64]) -> HashMap<i64, hierarchy_lookup_row> {
  const SQL_LOAD_BY_IDS: &str = "
    SELECT admin_level_id, user_friendly_name, json(ancestor_ids)
    FROM admin_levels_hierarchy
    WHERE admin_level_id IN
  ";

  if ids.is_empty() {
    return HashMap::new();
  }
  let sql = format!(
    "{} ({})",
    SQL_LOAD_BY_IDS.trim(),
    crate::database::placeholders_for(ids.len())
  );
  crate::database::query_by_ids(conn, &sql, ids.iter().copied(), |row| {
    let chain: String = row.get(2)?;
    Ok((
      row.get::<_, i64>(0)?,
      hierarchy_lookup_row {
        user_friendly_name: row.get(1)?,
        ancestor_ids: decode_chain(&chain),
      },
    ))
  })
  .into_iter()
  .collect()
}

pub fn batch_insert(conn: &Connection, rows: &[hierarchy_row]) {
  const SQL_INSERT: &str = "
    INSERT OR IGNORE INTO admin_levels_hierarchy (
      admin_level_id,
      ancestor_ids,
      user_friendly_name
    ) VALUES (
      ?1,
      jsonb(?2),
      ?3
    );
  ";

  if rows.is_empty() {
    return;
  }
  let tx = conn
    .unchecked_transaction()
    .expect("failed to begin transaction");
  {
    let mut stmt = tx
      .prepare(SQL_INSERT)
      .expect("failed to prepare hierarchy insert");
    rows
      .iter()
      .try_for_each(|row| {
        stmt
          .execute(rusqlite::params![
            row.admin_level_id,
            row.ancestor_ids,
            row.user_friendly_name
          ])
          .map(|_| ())
      })
      .expect("failed to insert hierarchy row");
  }
  tx.commit().expect("failed to commit");
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod tests;
