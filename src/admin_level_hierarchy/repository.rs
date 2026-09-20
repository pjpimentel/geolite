use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

use super::entity::{hierarchy_edges, node};
use crate::admin_level::level;
use crate::database::table;

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS admin_levels_hierarchy (
    admin_level_id INTEGER NOT NULL REFERENCES admin_levels(id) ON DELETE CASCADE,
    parent_id INTEGER REFERENCES admin_levels(id) ON DELETE CASCADE,
    PRIMARY KEY (admin_level_id, parent_id)
  );
";

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS admin_levels_hierarchy_search_by_parent
    ON admin_levels_hierarchy (parent_id, admin_level_id);
";

const SQL_DROP: &str = "DROP TABLE IF EXISTS admin_levels_hierarchy;";

// `IS` answers the roots for a null parent and the children for a given one, through the index
// in both cases; `=` would never match a null
const SQL_NODES_UNDER: &str = "
  SELECT al.id, al.admin_level, al.name
  FROM admin_levels al
  WHERE al.id IN (
    SELECT admin_level_id
    FROM admin_levels_hierarchy
    WHERE parent_id IS ?1
  )
  ORDER BY al.admin_level ASC, al.name ASC, al.id ASC
";

const SQL_INSERT_EDGE: &str = "
  INSERT OR IGNORE INTO admin_levels_hierarchy (
    admin_level_id,
    parent_id
  ) VALUES (
    ?1,
    ?2
  );
";

pub struct admin_levels_hierarchy;

impl table for admin_levels_hierarchy {
  const CREATE: &str = SQL_CREATE;
  const INDEXES: &str = SQL_CREATE_INDEXES;
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
    SELECT COUNT(*)
    FROM admin_levels al
    WHERE NOT EXISTS (
      SELECT 1
      FROM admin_levels_hierarchy h
      WHERE h.admin_level_id = al.id
    )
  ";

  conn
    .query_row(SQL_PENDING_TOTAL, [], |row| row.get::<_, i64>(0))
    .expect("failed to query pending total")
}

pub fn pending_street_ids(conn: &Connection) -> Vec<i64> {
  const SQL_PENDING_STREET_IDS: &str = "
    SELECT al.id
    FROM admin_levels al
    WHERE al.admin_level = ?1
      AND NOT EXISTS (
        SELECT 1
        FROM admin_levels_hierarchy h
        WHERE h.admin_level_id = al.id
      )
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

// the reads of the directory view: nothing in the binary walks the tree yet, the tui and the
// places api of the backlog will
#[allow(dead_code)]
pub fn roots(conn: &Connection) -> Vec<node> {
  nodes_under(conn, None)
}

#[allow(dead_code)]
pub fn children_of(conn: &Connection, parent_id: i64) -> Vec<node> {
  nodes_under(conn, Some(parent_id))
}

fn nodes_under(conn: &Connection, parent_id: Option<i64>) -> Vec<node> {
  let mut stmt = conn
    .prepare(SQL_NODES_UNDER)
    .expect("failed to prepare nodes under a parent");
  stmt
    .query_map([parent_id], |row| {
      let id: i64 = row.get(0)?;
      let raw: u8 = row.get(1)?;
      let name: String = row.get(2)?;
      Ok(level::new(raw).map(|level| node { id, level, name }))
    })
    .expect("failed to query nodes under a parent")
    .filter_map(|r| r.expect("failed to read a node"))
    .collect()
}

#[allow(dead_code)]
pub fn parents_of(conn: &Connection, id: i64) -> Vec<i64> {
  const SQL_PARENTS_OF: &str = "
    SELECT parent_id
    FROM admin_levels_hierarchy
    WHERE admin_level_id = ?1
      AND parent_id IS NOT NULL
    ORDER BY parent_id ASC
  ";

  let mut stmt = conn
    .prepare(SQL_PARENTS_OF)
    .expect("failed to prepare parents of an area");
  stmt
    .query_map([id], |row| row.get::<_, i64>(0))
    .expect("failed to query parents of an area")
    .map(|r| r.expect("failed to read a parent id"))
    .collect()
}

// the edges reachable upward from the ids, one query per layer up to the roots; an area without
// a row is absent from the map, which `paths_of` reads as a root
pub fn ancestry_of(conn: &Connection, ids: &[i64]) -> HashMap<i64, Vec<i64>> {
  const SQL_EDGES_OF: &str = "
    SELECT admin_level_id, parent_id
    FROM admin_levels_hierarchy
    WHERE admin_level_id IN
  ";

  let mut edges: HashMap<i64, Vec<i64>> = HashMap::new();
  let mut visited: HashSet<i64> = HashSet::new();
  let mut frontier: Vec<i64> = ids.to_vec();
  frontier.sort_unstable();
  frontier.dedup();
  while !frontier.is_empty() {
    visited.extend(frontier.iter().copied());
    let sql = format!(
      "{} ({})",
      SQL_EDGES_OF.trim(),
      crate::database::placeholders_for(frontier.len())
    );
    let rows: Vec<(i64, Option<i64>)> =
      crate::database::query_by_ids(conn, &sql, frontier.iter().copied(), |row| {
        Ok((row.get(0)?, row.get(1)?))
      });
    let mut next: Vec<i64> = Vec::new();
    for (child, parent) in rows {
      let parents = edges.entry(child).or_default();
      if let Some(parent) = parent {
        parents.push(parent);
        if !visited.contains(&parent) {
          next.push(parent);
        }
      }
    }
    next.sort_unstable();
    next.dedup();
    frontier = next;
  }
  for parents in edges.values_mut() {
    parents.sort_unstable();
  }
  edges
}

pub fn load_all_edges(conn: &Connection) -> HashMap<i64, Vec<i64>> {
  const SQL_LOAD_ALL_EDGES: &str = "
    SELECT admin_level_id, parent_id
    FROM admin_levels_hierarchy
    ORDER BY admin_level_id ASC, parent_id ASC
  ";

  let mut stmt = conn
    .prepare(SQL_LOAD_ALL_EDGES)
    .expect("failed to prepare load all edges");
  let mut edges: HashMap<i64, Vec<i64>> = HashMap::new();
  let rows = stmt
    .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)))
    .expect("failed to query all edges");
  for row in rows {
    let (child, parent) = row.expect("failed to read an edge");
    let parents = edges.entry(child).or_default();
    if let Some(parent) = parent {
      parents.push(parent);
    }
  }
  edges
}

pub fn batch_insert(conn: &Connection, rows: &[hierarchy_edges]) {
  // a null parent escapes the primary key, nulls never collide, so the root row is written only
  // where the area has no row yet
  const SQL_INSERT_ROOT: &str = "
    INSERT INTO admin_levels_hierarchy (
      admin_level_id,
      parent_id
    )
    SELECT ?1, NULL
    WHERE NOT EXISTS (
      SELECT 1
      FROM admin_levels_hierarchy
      WHERE admin_level_id = ?1
    );
  ";

  if rows.is_empty() {
    return;
  }
  let tx = conn
    .unchecked_transaction()
    .expect("failed to begin transaction");
  {
    let mut insert_edge = tx
      .prepare(SQL_INSERT_EDGE)
      .expect("failed to prepare hierarchy edge insert");
    let mut insert_root = tx
      .prepare(SQL_INSERT_ROOT)
      .expect("failed to prepare hierarchy root insert");
    for row in rows {
      if row.parents.is_empty() {
        insert_root
          .execute([row.admin_level_id])
          .expect("failed to insert hierarchy root");
        continue;
      }
      for parent in &row.parents {
        insert_edge
          .execute(rusqlite::params![row.admin_level_id, parent])
          .expect("failed to insert hierarchy edge");
      }
    }
  }
  tx.commit().expect("failed to commit");
}


pub fn replace_parents(conn: &Connection, rows: &[hierarchy_edges]) {
  const SQL_DELETE_EDGES: &str = "
    DELETE FROM admin_levels_hierarchy
    WHERE admin_level_id = ?1
  ";

  if rows.is_empty() {
    return;
  }
  let tx = conn
    .unchecked_transaction()
    .expect("failed to begin transaction");
  {
    let mut delete_edges = tx
      .prepare(SQL_DELETE_EDGES)
      .expect("failed to prepare hierarchy edge delete");
    let mut insert_edge = tx
      .prepare(SQL_INSERT_EDGE)
      .expect("failed to prepare hierarchy edge insert");
    for row in rows {
      delete_edges
        .execute([row.admin_level_id])
        .expect("failed to delete hierarchy edges");
      for parent in &row.parents {
        insert_edge
          .execute(rusqlite::params![row.admin_level_id, parent])
          .expect("failed to insert hierarchy edge");
      }
    }
  }
  tx.commit().expect("failed to commit");
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod tests;