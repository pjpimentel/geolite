use geo::{BoundingRect, Geometry};
use geozero::{CoordDimensions, ToGeo, ToWkb, wkb::SpatiaLiteWkb};
use rusqlite::Connection;
use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

pub struct admin_geometry(pub Geometry<f64>);

impl admin_geometry {
  pub fn geometry(&self) -> &Geometry<f64> {
    &self.0
  }

  pub fn into_geometry(self) -> Geometry<f64> {
    self.0
  }
}

impl From<Geometry<f64>> for admin_geometry {
  fn from(geometry: Geometry<f64>) -> Self {
    Self(geometry)
  }
}

impl ToSql for admin_geometry {
  fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
    let bbox = self.0.bounding_rect().ok_or_else(|| {
      rusqlite::Error::ToSqlConversionFailure(Box::<dyn std::error::Error + Send + Sync>::from(
        "admin_geometry has no bounding rect",
      ))
    })?;
    let envelope = vec![bbox.min().x, bbox.min().y, bbox.max().x, bbox.max().y];
    let blob = self
      .0
      .to_spatialite_wkb(CoordDimensions::default(), Some(4326), envelope)
      .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    Ok(ToSqlOutput::from(blob))
  }
}

impl FromSql for admin_geometry {
  fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
    let blob = value.as_blob()?;
    let empty = || {
      admin_geometry(Geometry::GeometryCollection(geo::GeometryCollection(
        vec![],
      )))
    };
    if blob.len() <= 40 {
      eprintln!(
        "warn: admin_geometry: blob too short ({} bytes); returning empty sentinel",
        blob.len()
      );
      return Ok(empty());
    }
    // spatialite format: use SpatiaLiteWkb on the full blob — geozero's to_spatialite_wkb
    // omits the byte-order byte from the WKB body and uses 0x69 as sub-geometry separator,
    // so Wkb (ISO WKB reader) cannot parse it; SpatiaLiteWkb handles the full blob correctly.
    match SpatiaLiteWkb(blob).to_geo() {
      Ok(geometry) => Ok(admin_geometry(geometry)),
      Err(e) => {
        let preview: Vec<String> = blob.iter().take(16).map(|b| format!("{:02x}", b)).collect();
        eprintln!(
          "warn: admin_geometry: WKB parse failed ({} bytes, head=[{}]): {:?}",
          blob.len(),
          preview.join(" "),
          e
        );
        Ok(empty())
      }
    }
  }
}

pub struct admin_levels {
  pub relation_id: Option<u64>,
  pub way_id: Option<u64>,
  pub admin_level: u8,
  pub wkb: admin_geometry,
  pub name: String,
  // country_iso_code: ISO 3166-1 alpha-2 (2 chars, e.g. 'BR', 'US')
  pub country_iso_code: Option<String>,
  pub post_code: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum admin_id_kind {
  way,
  relation,
}

pub fn pack_admin_id(kind: admin_id_kind, osm_id: u64) -> u64 {
  match kind {
    admin_id_kind::way => osm_id << 1,
    admin_id_kind::relation => (osm_id << 1) | 1,
  }
}

#[allow(dead_code)]
pub fn unpack_admin_id(id: u64) -> (admin_id_kind, u64) {
  let osm_id = id >> 1;
  let kind = if id & 1 == 1 {
    admin_id_kind::relation
  } else {
    admin_id_kind::way
  };
  (kind, osm_id)
}

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

const SQL_DROP: &str = "DROP TABLE IF EXISTS admin_levels;";

impl_table_ops!(pub(super), SQL_CREATE, SQL_DROP);

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS admin_levels_search_by_level
    ON admin_levels (admin_level);
";

pub fn create_indexes(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE_INDEXES)
    .expect("failed to create admin_levels indexes");
}

const SQL_DROP_INDEXES: &str = "
  DROP INDEX IF EXISTS admin_levels_search_by_level;
";

pub fn drop_indexes(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP_INDEXES)
    .expect("failed to drop admin_levels indexes");
}

// reads the centroid of a spatialite blob's MBR header without parsing the full geometry.
// layout: byte 0 = 0x00, byte 1 = endianness, bytes 2-5 = SRID, bytes 6-37 = MBR
// (min_x, min_y, max_x, max_y as four f64), byte 38 = 0x7C. returns (lon, lat) of the center.
pub fn mbr_center(blob: &[u8]) -> Option<(f64, f64)> {
  if blob.len() < 38 || blob[0] != 0x00 {
    return None;
  }
  let little_endian = blob[1] == 0x01;
  let read = |offset: usize| -> f64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&blob[offset..offset + 8]);
    if little_endian {
      f64::from_le_bytes(buf)
    } else {
      f64::from_be_bytes(buf)
    }
  };
  let min_x = read(6);
  let min_y = read(14);
  let max_x = read(22);
  let max_y = read(30);
  Some(((min_x + max_x) / 2.0, (min_y + max_y) / 2.0))
}

pub struct admin_level_geom_row {
  pub id: i64,
  pub admin_level: u8,
  pub name: String,
  pub wkb: Option<admin_geometry>,
  pub post_code: Option<String>,
}

// street = 12; levels with lower numbers are ancestors (countries, states, cities)
// loaded into memory for hierarchy resolution
const STREET_LEVEL: u8 = 12;

const SQL_PENDING_TOTAL: &str = "
  WITH pending AS (
    SELECT al.id
    FROM admin_levels al
    LEFT JOIN admin_levels_hierarchy h ON al.id = h.admin_level_id
    WHERE h.admin_level_id IS NULL
  )
  SELECT COUNT(*) FROM pending
";

pub fn pending_total(conn: &Connection) -> i64 {
  conn
    .query_row(SQL_PENDING_TOTAL, [], |row| row.get::<_, i64>(0))
    .expect("failed to query pending total")
}

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

pub fn load_all_below_street(conn: &Connection) -> Vec<admin_level_geom_row> {
  let mut stmt = conn
    .prepare(SQL_LOAD_ALL_BELOW_STREET)
    .expect("failed to prepare load ancestors");
  stmt
    .query_map([STREET_LEVEL], |row| {
      Ok(admin_level_geom_row {
        id: row.get(0)?,
        admin_level: row.get(1)?,
        name: row.get(2)?,
        wkb: row.get(3)?,
        post_code: row.get(4)?,
      })
    })
    .expect("failed to query ancestors")
    .map(|r| r.expect("failed to read ancestor row"))
    .collect()
}

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

pub fn pending_street_ids(conn: &Connection) -> Vec<i64> {
  let mut stmt = conn
    .prepare(SQL_PENDING_STREET_IDS)
    .expect("failed to prepare pending streets");
  stmt
    .query_map([STREET_LEVEL], |row| row.get::<_, i64>(0))
    .expect("failed to query pending streets")
    .map(|r| r.expect("failed to read street id"))
    .collect()
}

pub struct street_query_row {
  pub id: i64,
  pub admin_level: u8,
  pub wkb: Option<admin_geometry>,
}

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
    AND al.admin_level = 12
";

pub fn streets_for_coordinates(
  conn: &Connection,
  lon: f64,
  lat: f64,
  delta: f64,
  bbox: crate::query::bounding_box,
) -> Vec<street_query_row> {
  let map_row = |row: &rusqlite::Row| {
    Ok(street_query_row {
      id: row.get(0)?,
      admin_level: row.get(1)?,
      wkb: row.get(2)?,
    })
  };
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
        bbox.min_lat
      ],
      map_row,
    )
    .expect("failed to query streets for coordinates")
    .map(|r| r.expect("failed to read street row"))
    .collect()
}

const SQL_IDS_IN_BOUNDING_BOX: &str = "
  SELECT id
  FROM admin_levels_rtree
  WHERE min_lon <= ?1 AND max_lon >= ?2
    AND min_lat <= ?3 AND max_lat >= ?4
";

// todos os ids cuja geometria (bbox) intersecta o envelope. usado para restringir o ranking
// textual à região no tantivy (espacial-primeiro), em vez de filtrar depois do corte do fts
pub fn ids_in_bounding_box(conn: &Connection, bbox: crate::query::bounding_box) -> Vec<i64> {
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

const SQL_LOAD_BY_IDS_PREFIX: &str = "
  SELECT
    id,
    admin_level,
    name,
    wkb,
    post_code
  FROM admin_levels
  WHERE id IN
";

pub fn load_by_ids(conn: &Connection, ids: &[i64]) -> Vec<admin_level_geom_row> {
  if ids.is_empty() {
    return vec![];
  }
  let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
  let sql = format!("{} ({placeholders})", SQL_LOAD_BY_IDS_PREFIX.trim());
  let params: Vec<rusqlite::types::Value> = ids
    .iter()
    .map(|&id| rusqlite::types::Value::Integer(id))
    .collect();
  let mut stmt = conn.prepare(&sql).expect("failed to prepare load by ids");
  stmt
    .query_map(rusqlite::params_from_iter(params.iter()), |row| {
      Ok(admin_level_geom_row {
        id: row.get(0)?,
        admin_level: row.get(1)?,
        name: row.get(2)?,
        wkb: row.get(3)?,
        post_code: row.get(4)?,
      })
    })
    .expect("failed to query by ids")
    .map(|r| r.expect("failed to read row by id"))
    .collect()
}

pub struct admin_area_row {
  pub id: i64,
  pub name: String,
  pub admin_level: u8,
  pub relation_id: Option<u64>,
  pub way_id: Option<u64>,
  pub wkb: Option<admin_geometry>,
}

fn map_admin_area_row(row: &rusqlite::Row) -> rusqlite::Result<admin_area_row> {
  Ok(admin_area_row {
    id: row.get(0)?,
    name: row.get(1)?,
    admin_level: row.get(2)?,
    relation_id: row.get(3)?,
    way_id: row.get(4)?,
    wkb: row.get(5)?,
  })
}

const SQL_LOAD_FULL_BY_IDS_PREFIX: &str = "
  SELECT id, name, admin_level, relation_id, way_id, wkb
  FROM admin_levels
  WHERE id IN
";

pub fn load_full_by_ids(conn: &Connection, ids: &[i64]) -> Vec<admin_area_row> {
  if ids.is_empty() {
    return vec![];
  }
  let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
  let sql = format!("{} ({placeholders})", SQL_LOAD_FULL_BY_IDS_PREFIX.trim());
  let params: Vec<rusqlite::types::Value> = ids
    .iter()
    .map(|&id| rusqlite::types::Value::Integer(id))
    .collect();
  let mut stmt = conn
    .prepare(&sql)
    .expect("failed to prepare load_full_by_ids");
  stmt
    .query_map(
      rusqlite::params_from_iter(params.iter()),
      map_admin_area_row,
    )
    .expect("failed to query load_full_by_ids")
    .map(|r| r.expect("failed to read load_full_by_ids row"))
    .collect()
}

pub struct admin_meta_row {
  pub id: i64,
  pub name: String,
  pub admin_level: u8,
  pub relation_id: Option<u64>,
  pub way_id: Option<u64>,
  pub country_iso_code: Option<String>,
  pub post_code: Option<String>,
}

const SQL_LOAD_METADATA_BY_IDS_PREFIX: &str = "
  SELECT id, name, admin_level, relation_id, way_id, country_iso_code, post_code
  FROM admin_levels
  WHERE id IN
";

pub fn load_metadata_by_ids(
  conn: &Connection,
  ids: &[i64],
) -> std::collections::HashMap<i64, admin_meta_row> {
  if ids.is_empty() {
    return std::collections::HashMap::new();
  }
  let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
  let sql = format!(
    "{} ({placeholders})",
    SQL_LOAD_METADATA_BY_IDS_PREFIX.trim()
  );
  let params: Vec<rusqlite::types::Value> = ids
    .iter()
    .map(|&id| rusqlite::types::Value::Integer(id))
    .collect();
  let mut stmt = conn
    .prepare(&sql)
    .expect("failed to prepare load_metadata_by_ids");
  stmt
    .query_map(rusqlite::params_from_iter(params.iter()), |row| {
      Ok(admin_meta_row {
        id: row.get(0)?,
        name: row.get(1)?,
        admin_level: row.get(2)?,
        relation_id: row.get(3)?,
        way_id: row.get(4)?,
        country_iso_code: row.get(5)?,
        post_code: row.get(6)?,
      })
    })
    .expect("failed to query load_metadata_by_ids")
    .map(|r| r.expect("failed to read admin_meta_row"))
    .map(|r| (r.id, r))
    .collect()
}

const SQL_COUNT_WITH_GEOMETRY: &str = "
  SELECT COUNT(*)
  FROM admin_levels
  WHERE wkb IS NOT NULL
";

pub fn count_with_geometry(conn: &Connection) -> i64 {
  conn
    .query_row(SQL_COUNT_WITH_GEOMETRY, [], |row| row.get::<_, i64>(0))
    .expect("failed to count admin_levels with geometry")
}

const SQL_ID_RANGE_WITH_GEOMETRY: &str = "
  SELECT MIN(id), MAX(id)
  FROM admin_levels
  WHERE wkb IS NOT NULL
";

pub fn id_range_with_geometry(conn: &Connection) -> (i64, i64) {
  conn
    .query_row(SQL_ID_RANGE_WITH_GEOMETRY, [], |row| {
      Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
    })
    .expect("failed to query id range with geometry")
}

const SQL_LOAD_WKB_PAGE: &str = "
  SELECT id, wkb
  FROM admin_levels
  WHERE wkb IS NOT NULL
    AND id > ?1
    AND id <= ?2
  ORDER BY id ASC
  LIMIT ?3
";

pub fn load_wkb_page(
  conn: &Connection,
  last_id: i64,
  max_id: i64,
  limit: usize,
) -> Vec<(i64, admin_geometry)> {
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

const SQL_CREATE_RTREE: &str = "
  CREATE VIRTUAL TABLE IF NOT EXISTS admin_levels_rtree
  USING rtree(id, min_lon, max_lon, min_lat, max_lat);
";

const SQL_DROP_RTREE: &str = "DROP TABLE IF EXISTS admin_levels_rtree;";

const SQL_INSERT_RTREE: &str = "
  INSERT INTO admin_levels_rtree (
    id,
    min_lon,
    max_lon,
    min_lat,
    max_lat
  ) VALUES (
    ?1,
    ?2,
    ?3,
    ?4,
    ?5
  );
";

pub struct rtree_row {
  pub id: i64,
  pub min_lon: f64,
  pub max_lon: f64,
  pub min_lat: f64,
  pub max_lat: f64,
}

pub(super) fn create_rtree(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE_RTREE)
    .expect("failed to create admin_levels_rtree");
}

pub(super) fn drop_rtree(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP_RTREE)
    .expect("failed to drop admin_levels_rtree");
}

pub fn recreate_rtree(conn: &Connection) {
  drop_rtree(conn);
  create_rtree(conn);
}

pub fn batch_insert_rtree(conn: &Connection, rows: &[rtree_row]) {
  if rows.is_empty() {
    return;
  }
  let tx = conn
    .unchecked_transaction()
    .expect("failed to begin transaction");
  {
    let mut stmt = tx
      .prepare(SQL_INSERT_RTREE)
      .expect("failed to prepare rtree insert");
    rows
      .iter()
      .try_for_each(|row| {
        stmt
          .execute(rusqlite::params![
            row.id,
            row.min_lon,
            row.max_lon,
            row.min_lat,
            row.max_lat
          ])
          .map(|_| ())
      })
      .expect("failed to insert rtree row");
  }
  tx.commit().expect("failed to commit rtree batch");
}

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

pub fn batch_upsert(conn: &Connection, rows: &[admin_levels]) -> i64 {
  let tx = conn
    .unchecked_transaction()
    .expect("failed to begin transaction");
  let mut total_changes: i64 = 0;
  {
    let mut stmt = tx.prepare(SQL_UPSERT).expect("failed to prepare upsert");
    for row in rows {
      let id: u64 = match (row.relation_id, row.way_id) {
        (Some(rel), _) => pack_admin_id(admin_id_kind::relation, rel),
        (None, Some(w)) => pack_admin_id(admin_id_kind::way, w),
        (None, None) => panic!(
          "admin_levels row has neither way_id nor relation_id; \
           cannot derive a stable id (admin_level={}, name={:?})",
          row.admin_level, row.name,
        ),
      };
      let changes = stmt
        .execute(rusqlite::params![
          id,
          row.relation_id,
          row.way_id,
          row.admin_level,
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
#[path = "admin_levels.test.rs"]
mod tests;
