use rusqlite::Connection;

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS admin_levels_hierarchy (
    admin_level_id INTEGER PRIMARY KEY REFERENCES admin_levels(id) ON DELETE CASCADE,
    ancestor_ids BLOB NOT NULL,
    user_friendly_name TEXT NOT NULL
  );
";

const SQL_DROP: &str = "DROP TABLE IF EXISTS admin_levels_hierarchy;";

const SQL_COUNT: &str = "
  SELECT COUNT(*) FROM admin_levels_hierarchy
";

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

impl_table_ops!(pub(super), SQL_CREATE, SQL_DROP);

pub struct hierarchy_row {
  pub admin_level_id: i64,
  pub ancestor_ids: String,
  pub user_friendly_name: String,
}

pub struct hierarchy_lookup_row {
  pub user_friendly_name: String,
  pub ancestor_ids: Vec<i64>,
}

pub fn destroy(conn: &Connection) {
  drop_table(conn);
  create_table(conn);
}

pub fn count(conn: &Connection) -> i64 {
  conn
    .query_row(SQL_COUNT, [], |row| row.get(0))
    .expect("failed to count admin_levels_hierarchy")
}

pub fn load_by_ids(
  conn: &Connection,
  ids: &[i64],
) -> std::collections::HashMap<i64, hierarchy_lookup_row> {
  if ids.is_empty() {
    return std::collections::HashMap::new();
  }
  let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
  let sql = format!(
    "SELECT admin_level_id, user_friendly_name, json(ancestor_ids)
         FROM admin_levels_hierarchy
         WHERE admin_level_id IN ({placeholders})"
  );
  let params: Vec<rusqlite::types::Value> = ids
    .iter()
    .map(|&id| rusqlite::types::Value::Integer(id))
    .collect();
  let mut stmt = conn
    .prepare(&sql)
    .expect("failed to prepare hierarchy load_by_ids");
  stmt
    .query_map(rusqlite::params_from_iter(params.iter()), |row| {
      let json_str: String = row.get(2)?;
      Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, json_str))
    })
    .expect("failed to query hierarchy load_by_ids")
    .filter_map(|r| r.ok())
    .map(|(id, name, json_str)| {
      let ancestor_ids: Vec<i64> = serde_json::from_str(&json_str).unwrap_or_default();
      (
        id,
        hierarchy_lookup_row {
          user_friendly_name: name,
          ancestor_ids,
        },
      )
    })
    .collect()
}

pub fn batch_insert(conn: &Connection, rows: &[hierarchy_row]) {
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
