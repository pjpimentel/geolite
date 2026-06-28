use rusqlite::Connection;

use super::admin_levels::admin_geometry;

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

impl_table_ops!(pub(super), SQL_CREATE, SQL_DROP);

const SQL_STREETS_WITH_GEOMETRY: &str = "
  SELECT id, name, wkb
  FROM admin_levels
  WHERE admin_level = 12
  AND wkb IS NOT NULL;
";

const SQL_LOAD_ALL_CANDIDATES: &str = "
  WITH raw AS (
    SELECT
      id,
      TRIM({number_select}) AS number,
      {street_select} AS addr_street,
      CAST(payload->>'lon' AS REAL) AS lon,
      CAST(payload->>'lat' AS REAL) AS lat
    FROM osm_data.osm_nodes
    WHERE {number_select} IS NOT NULL
  )
  SELECT
    id,
    CASE
      WHEN SUBSTR(number, -1, 1) GLOB '[A-Za-z]'
        AND (SUBSTR(number, -2, 1) = ' ' OR SUBSTR(number, -2, 1) = '-')
        AND SUBSTR(number, 1, LENGTH(number) - 2) <> ''
        AND SUBSTR(number, 1, LENGTH(number) - 2) NOT GLOB '*[^0-9]*'
        THEN SUBSTR(number, 1, LENGTH(number) - 2) || UPPER(SUBSTR(number, -1, 1))
      WHEN SUBSTR(number, -1, 1) GLOB '[A-Za-z]'
        AND SUBSTR(number, 1, LENGTH(number) - 1) <> ''
        AND SUBSTR(number, 1, LENGTH(number) - 1) NOT GLOB '*[^0-9]*'
        THEN SUBSTR(number, 1, LENGTH(number) - 1) || UPPER(SUBSTR(number, -1, 1))
      ELSE number
    END AS number,
    addr_street,
    lon,
    lat
  FROM raw
  WHERE number <> ''
  {drop_clause}
";


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

pub struct house_numbers {
  pub node_id: u64,
  pub admin_level_id: i64,
  pub number: String,
  pub wkb: admin_geometry,
  pub strategy: u8,
}

pub fn create_indexes(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE_INDEXES)
    .expect("failed to create house_numbers indexes");
}

const SQL_DROP_INDEXES: &str = "
  DROP INDEX IF EXISTS house_numbers_search_by_admin_level_id;
";

pub fn drop_indexes(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP_INDEXES)
    .expect("failed to drop house_numbers indexes");
}

pub struct street_meta_row {
  pub id: i64,
  pub name: String,
  // coarse centroid (lon, lat) read from the geometry's MBR — used only to bucket streets into
  // tiles during house-number matching, so MBR-center precision is sufficient.
  pub cx: f64,
  pub cy: f64,
}

pub fn streets_with_centroid(conn: &Connection) -> Vec<street_meta_row> {
  let mut stmt = conn
    .prepare(SQL_STREETS_WITH_GEOMETRY)
    .expect("failed to prepare streets with geometry");
  stmt
    .query_map([], |row| {
      let id: i64 = row.get(0)?;
      let name: String = row.get(1)?;
      let blob: Vec<u8> = row.get(2)?;
      Ok((id, name, blob))
    })
    .expect("failed to query streets")
    .filter_map(|r| {
      let (id, name, blob) = r.expect("failed to read street meta row");
      let (cx, cy) = super::admin_levels::mbr_center(&blob)?;
      Some(street_meta_row { id, name, cx, cy })
    })
    .collect()
}

pub struct street_wkb_row {
  pub id: i64,
  pub wkb: admin_geometry,
}

pub fn streets_wkb_by_ids(conn: &Connection, ids: &[i64]) -> Vec<street_wkb_row> {
  if ids.is_empty() {
    return vec![];
  }
  let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
  let sql =
    format!("SELECT id, wkb FROM admin_levels WHERE id IN ({placeholders}) AND wkb IS NOT NULL");
  let params: Vec<rusqlite::types::Value> = ids
    .iter()
    .map(|&id| rusqlite::types::Value::Integer(id))
    .collect();
  let mut stmt = conn
    .prepare(&sql)
    .expect("failed to prepare streets wkb by ids");
  stmt
    .query_map(rusqlite::params_from_iter(params.iter()), |row| {
      Ok(street_wkb_row {
        id: row.get(0)?,
        wkb: row.get(1)?,
      })
    })
    .expect("failed to query streets wkb")
    .map(|r| r.expect("failed to read street wkb row"))
    .collect()
}

pub struct candidate_row {
  pub id: u64,
  pub number: String,
  pub addr_street: Option<String>,
  pub lon: f64,
  pub lat: f64,
}

pub fn load_all_candidates(
  conn: &Connection,
  housenumber_tags: &[&str],
  street_tags: &[&str],
  drop_values: &[&str],
) -> Vec<candidate_row> {
  debug_assert!(!housenumber_tags.is_empty(), "housenumber_tags must not be empty");
  let number_select = super::build_name_select("payload", housenumber_tags);
  let street_select = super::build_name_select("payload", street_tags);
  let drop_clause = if drop_values.is_empty() {
    String::new()
  } else {
    let placeholders = drop_values.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    format!("AND LOWER(number) NOT IN ({placeholders})")
  };
  let sql = SQL_LOAD_ALL_CANDIDATES
    .replace("{number_select}", &number_select)
    .replace("{street_select}", &street_select)
    .replace("{drop_clause}", &drop_clause);
  let params: Vec<rusqlite::types::Value> = drop_values
    .iter()
    .map(|d| rusqlite::types::Value::Text(d.to_lowercase()))
    .collect();
  let mut stmt = conn
    .prepare(&sql)
    .expect("failed to prepare load all candidates");
  stmt
    .query_map(rusqlite::params_from_iter(params.iter()), |row| {
      Ok(candidate_row {
        id: row.get(0)?,
        number: row.get(1)?,
        addr_street: row.get(2)?,
        lon: row.get(3)?,
        lat: row.get(4)?,
      })
    })
    .expect("failed to query candidates")
    .map(|r| r.expect("failed to read candidate row"))
    .collect()
}

pub struct hn_for_street {
  pub admin_level_id: i64,
  pub number: String,
  pub wkb: Option<admin_geometry>,
}

pub fn by_admin_level_ids(conn: &Connection, ids: &[i64]) -> Vec<hn_for_street> {
  if ids.is_empty() {
    return vec![];
  }
  let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
  let sql = format!(
    "SELECT admin_level_id, number, wkb
       FROM house_numbers WHERE admin_level_id IN ({placeholders})"
  );
  let params: Vec<rusqlite::types::Value> = ids
    .iter()
    .map(|&id| rusqlite::types::Value::Integer(id))
    .collect();
  let mut stmt = conn
    .prepare(&sql)
    .expect("failed to prepare by_admin_level_ids");
  stmt
    .query_map(rusqlite::params_from_iter(params.iter()), |row| {
      Ok(hn_for_street {
        admin_level_id: row.get(0)?,
        number: row.get(1)?,
        wkb: row.get(2)?,
      })
    })
    .expect("failed to query by_admin_level_ids")
    .map(|r| r.expect("failed to read hn_for_street row"))
    .collect()
}

pub fn batch_insert(conn: &Connection, rows: &[house_numbers]) -> i64 {
  if rows.is_empty() {
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
    for row in rows {
      let changes = stmt
        .execute(rusqlite::params![
          row.node_id,
          row.admin_level_id,
          row.number,
          row.wkb,
          row.strategy,
        ])
        .expect("failed to insert house_number");
      total += changes as i64;
    }
  }
  tx.commit().expect("failed to commit");
  total
}

#[cfg(test)]
#[path = "house_numbers.test.rs"]
mod tests;
