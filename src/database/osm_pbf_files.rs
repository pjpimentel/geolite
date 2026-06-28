use rusqlite::Connection;

#[allow(dead_code)]
pub struct osm_pbf_files {
  pub id: Option<u32>,

  pub geofabrik_id: Option<String>,
  pub geofabrik_name: Option<String>,
  pub geofabrik_parent: Option<String>,
  pub geofabrik_url: Option<String>,
  pub geofabrik_wkt: Option<Vec<u8>>,

  pub file_path: Option<String>,
  pub size_bytes: Option<u64>,
  pub md5: Option<String>,
  pub downloaded_at: Option<i64>,

  pub osm_data_extracted_at: Option<i64>,

  pub osm_header_bbox_wkt: Option<Vec<u8>>,
  pub osm_header_required_features: Option<Vec<String>>,
  pub osm_header_optional_features: Option<Vec<String>>,
  pub osm_header_writingprogram: Option<String>,
  pub osm_header_source: Option<String>,
  pub osm_header_osmosis_replication_timestamp: Option<i64>,
  pub osm_header_osmosis_replication_sequence_number: Option<u32>,
  pub osm_header_osmosis_replication_base_url: Option<String>,

  pub node_count: Option<u64>,
  pub way_count: Option<u64>,
  pub relation_count: Option<u64>,

  pub admin_levels_count: Option<i64>,
  pub house_numbers_count: Option<i64>,

  pub created_at: Option<i64>,
  pub updated_at: Option<i64>,
}

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS osm_pbf_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    geofabrik_id VARCHAR(128) UNIQUE,
    geofabrik_name VARCHAR(128),
    geofabrik_parent VARCHAR(128),
    geofabrik_url VARCHAR(512),
    geofabrik_wkt BLOB,
    file_path VARCHAR(1024) UNIQUE,
    size_bytes INTEGER,
    md5 VARCHAR(32),
    downloaded_at INTEGER,
    osm_header_bbox_wkt BLOB,
    osm_header_required_features BLOB,
    osm_header_optional_features BLOB,
    osm_header_writingprogram VARCHAR(128),
    osm_header_source VARCHAR(512),
    osm_header_osmosis_replication_timestamp INTEGER,
    osm_header_osmosis_replication_sequence_number INTEGER,
    osm_header_osmosis_replication_base_url VARCHAR(512),
    osm_data_extracted_at INTEGER,
    node_count INTEGER,
    way_count INTEGER,
    relation_count INTEGER,
    admin_levels_count INTEGER,
    house_numbers_count INTEGER,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
  );
  CREATE TRIGGER IF NOT EXISTS osm_pbf_files_updated_at
    AFTER UPDATE ON osm_pbf_files
    FOR EACH ROW
    BEGIN
      UPDATE osm_pbf_files SET updated_at = unixepoch() WHERE id = OLD.id;
    END;
";

const SQL_DROP: &str = "DROP TABLE IF EXISTS osm_pbf_files;";

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS osm_pbf_files_search_by_geofabrik_url
    ON osm_pbf_files(geofabrik_url);
";

impl_table_ops!(pub(super), SQL_CREATE, SQL_DROP);

pub(crate) fn create_indexes(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE_INDEXES)
    .expect("failed to create osm_pbf_files indexes");
}

const SQL_FILL_GEOFABRIK_BY_URL: &str = "
  UPDATE osm_pbf_files SET
    geofabrik_id = ?1,
    geofabrik_name = ?2,
    geofabrik_parent = ?3
  WHERE geofabrik_url = ?4
    AND geofabrik_id IS NULL
";

const SQL_UPSERT_GEOFABRIK_BY_ID: &str = "
  INSERT INTO osm_pbf_files (
    geofabrik_id,
    geofabrik_name,
    geofabrik_parent,
    geofabrik_url
  ) VALUES (
    ?1,
    ?2,
    ?3,
    ?4
  )
  ON CONFLICT(geofabrik_id) DO UPDATE SET
    geofabrik_name = excluded.geofabrik_name,
    geofabrik_parent = excluded.geofabrik_parent,
    geofabrik_url = excluded.geofabrik_url
";

pub(crate) fn upsert_geofabrik_index_item(
  conn: &Connection,
  geofabrik_id: &str,
  name: &str,
  parent: Option<&str>,
  url: &str,
) {
  let filled = conn
    .execute(
      SQL_FILL_GEOFABRIK_BY_URL,
      rusqlite::params![geofabrik_id, name, parent, url],
    )
    .expect("failed to fill geofabrik metadata by url");
  if filled == 0 {
    conn
      .execute(
        SQL_UPSERT_GEOFABRIK_BY_ID,
        rusqlite::params![geofabrik_id, name, parent, url],
      )
      .expect("failed to upsert geofabrik index item");
  }
}

const SQL_LIST_GEOFABRIK_INDEX: &str = "
  SELECT
    geofabrik_id,
    geofabrik_name,
    geofabrik_url
  FROM osm_pbf_files
  WHERE geofabrik_id IS NOT NULL
  ORDER BY geofabrik_id
";

pub(crate) fn list_geofabrik_index(conn: &Connection) -> Vec<(String, String, String)> {
  conn
    .prepare(SQL_LIST_GEOFABRIK_INDEX)
    .expect("failed to prepare list_geofabrik_index")
    .query_map([], |row| {
      Ok((
        row.get::<_, String>(0)?,
        row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        row.get::<_, Option<String>>(2)?.unwrap_or_default(),
      ))
    })
    .expect("failed to query geofabrik index")
    .collect::<Result<Vec<_>, _>>()
    .expect("failed to collect geofabrik index rows")
}

const SQL_GET_GEOFABRIK_URL_BY_ID: &str = "
  SELECT geofabrik_url
  FROM osm_pbf_files
  WHERE geofabrik_id = ?1
  LIMIT 1
";

pub(crate) fn get_geofabrik_url(conn: &Connection, geofabrik_id: &str) -> Option<String> {
  conn
    .query_row(
      SQL_GET_GEOFABRIK_URL_BY_ID,
      rusqlite::params![geofabrik_id],
      |row| row.get::<_, Option<String>>(0),
    )
    .ok()
    .flatten()
}

const SQL_ENSURE_FILE_PATH: &str = "INSERT OR IGNORE INTO osm_pbf_files (file_path) VALUES (?1)";

const SQL_GET_ID_BY_FILE_PATH: &str = "SELECT id FROM osm_pbf_files WHERE file_path = ?1";

pub(crate) fn ensure_by_file_path(conn: &Connection, file_path: &str) -> u32 {
  conn
    .execute(SQL_ENSURE_FILE_PATH, rusqlite::params![file_path])
    .expect("failed to ensure osm_pbf_files row");
  conn
    .query_row(
      SQL_GET_ID_BY_FILE_PATH,
      rusqlite::params![file_path],
      |row| row.get(0),
    )
    .expect("failed to get id after ensure")
}

const SQL_GET_FILE_PATH: &str = "
  SELECT file_path
  FROM osm_pbf_files
  WHERE (geofabrik_id = ?1 OR CAST(id AS TEXT) = ?1)
    AND file_path IS NOT NULL
  LIMIT 1
";

pub(crate) fn get_file_path(conn: &Connection, id_or_geofabrik_id: &str) -> Option<String> {
  conn
    .query_row(
      SQL_GET_FILE_PATH,
      rusqlite::params![id_or_geofabrik_id],
      |row| row.get(0),
    )
    .ok()
    .flatten()
}

const SQL_UPDATE_OSM_HEADER: &str = "
  UPDATE osm_pbf_files SET
    osm_header_bbox_wkt = ?1,
    osm_header_required_features = ?2,
    osm_header_optional_features = ?3,
    osm_header_writingprogram = ?4,
    osm_header_source = ?5,
    osm_header_osmosis_replication_timestamp = ?6,
    osm_header_osmosis_replication_sequence_number = ?7,
    osm_header_osmosis_replication_base_url = ?8
  WHERE file_path = ?9
";

#[allow(clippy::too_many_arguments)]
pub(crate) fn update_osm_header(
  conn: &Connection,
  osm_pbf_file_path: &str,
  bbox_wkt: Option<Vec<u8>>,
  required_features: Option<Vec<u8>>,
  optional_features: Option<Vec<u8>>,
  writingprogram: Option<&str>,
  source: Option<&str>,
  osmosis_replication_timestamp: Option<i64>,
  osmosis_replication_sequence_number: Option<u32>,
  osmosis_replication_base_url: Option<&str>,
) {
  ensure_by_file_path(conn, osm_pbf_file_path);
  conn
    .execute(
      SQL_UPDATE_OSM_HEADER,
      rusqlite::params![
        bbox_wkt,
        required_features,
        optional_features,
        writingprogram,
        source,
        osmosis_replication_timestamp,
        osmosis_replication_sequence_number,
        osmosis_replication_base_url,
        osm_pbf_file_path,
      ],
    )
    .expect("failed to update osm header");
}

const SQL_UPDATE_COUNTS: &str = "
  UPDATE osm_pbf_files SET
    node_count = ?1,
    way_count = ?2,
    relation_count = ?3,
    osm_data_extracted_at = unixepoch()
  WHERE id = ?4
";

pub(crate) fn update_counts(
  conn: &Connection,
  file_id: u32,
  node_count: u64,
  way_count: u64,
  relation_count: u64,
) {
  conn
    .execute(
      SQL_UPDATE_COUNTS,
      rusqlite::params![node_count, way_count, relation_count, file_id],
    )
    .expect("failed to update osm_pbf_files counts");
}

const SQL_UPDATE_DOWNLOADED: &str = "
  UPDATE osm_pbf_files SET
    file_path = ?1,
    size_bytes = ?2,
    md5 = ?3,
    downloaded_at = unixepoch()
  WHERE geofabrik_url = ?4
";

const SQL_INSERT_DOWNLOADED: &str = "
  INSERT OR IGNORE INTO osm_pbf_files (
    geofabrik_url,
    file_path,
    size_bytes,
    md5,
    downloaded_at
  ) VALUES (
    ?1,
    ?2,
    ?3,
    ?4,
    unixepoch()
  )
";

pub(crate) fn update_downloaded(
  conn: &Connection,
  url: &str,
  file_path: &str,
  size_bytes: u64,
  md5: &str,
) {
  let affected = conn
    .execute(
      SQL_UPDATE_DOWNLOADED,
      rusqlite::params![file_path, size_bytes, md5, url],
    )
    .expect("failed to update downloaded");
  if affected == 0 {
    conn
      .execute(
        SQL_INSERT_DOWNLOADED,
        rusqlite::params![url, file_path, size_bytes, md5],
      )
      .expect("failed to insert downloaded");
  }
}

const SQL_UPDATE_ADMIN_LEVELS_COUNT: &str = "
  WITH way_to_file AS (
    SELECT
      osm_data.osm_ways.id AS osm_id,
      osm_data.osm_pbf_blob_chunks.file_id AS file_id
    FROM osm_data.osm_ways
    JOIN osm_data.osm_pbf_blob_chunks
      ON osm_data.osm_pbf_blob_chunks.id = osm_data.osm_ways.osm_pbf_chunk_id
  ),
  relation_to_file AS (
    SELECT
      osm_data.osm_relations.id AS osm_id,
      osm_data.osm_pbf_blob_chunks.file_id AS file_id
    FROM osm_data.osm_relations
    JOIN osm_data.osm_pbf_blob_chunks
      ON osm_data.osm_pbf_blob_chunks.id = osm_data.osm_relations.osm_pbf_chunk_id
  ),
  admin_to_file AS (
    SELECT
      admin_levels.id AS admin_id,
      COALESCE(way_to_file.file_id, relation_to_file.file_id) AS file_id
    FROM admin_levels
    LEFT JOIN way_to_file ON way_to_file.osm_id = admin_levels.way_id
    LEFT JOIN relation_to_file ON relation_to_file.osm_id = admin_levels.relation_id
    WHERE admin_levels.wkb IS NOT NULL
  ),
  counts_per_file AS (
    SELECT
      file_id,
      COUNT(*) AS total
    FROM admin_to_file
    WHERE file_id IS NOT NULL
    GROUP BY file_id
  )
  UPDATE osm_pbf_files SET
    admin_levels_count = COALESCE(
      (SELECT total FROM counts_per_file WHERE counts_per_file.file_id = osm_pbf_files.id),
      0
    )
  WHERE file_path IS NOT NULL
";

pub(crate) fn update_admin_levels_count(conn: &Connection) {
  conn
    .execute(SQL_UPDATE_ADMIN_LEVELS_COUNT, [])
    .expect("failed to update osm_pbf_files admin_levels_count");
}

const SQL_UPDATE_HOUSE_NUMBERS_COUNT: &str = "
  WITH way_to_file AS (
    SELECT
      osm_data.osm_ways.id AS osm_id,
      osm_data.osm_pbf_blob_chunks.file_id AS file_id
    FROM osm_data.osm_ways
    JOIN osm_data.osm_pbf_blob_chunks
      ON osm_data.osm_pbf_blob_chunks.id = osm_data.osm_ways.osm_pbf_chunk_id
  ),
  relation_to_file AS (
    SELECT
      osm_data.osm_relations.id AS osm_id,
      osm_data.osm_pbf_blob_chunks.file_id AS file_id
    FROM osm_data.osm_relations
    JOIN osm_data.osm_pbf_blob_chunks
      ON osm_data.osm_pbf_blob_chunks.id = osm_data.osm_relations.osm_pbf_chunk_id
  ),
  admin_to_file AS (
    SELECT
      admin_levels.id AS admin_id,
      COALESCE(way_to_file.file_id, relation_to_file.file_id) AS file_id
    FROM admin_levels
    LEFT JOIN way_to_file ON way_to_file.osm_id = admin_levels.way_id
    LEFT JOIN relation_to_file ON relation_to_file.osm_id = admin_levels.relation_id
  ),
  counts_per_file AS (
    SELECT
      admin_to_file.file_id AS file_id,
      COUNT(*) AS total
    FROM house_numbers
    JOIN admin_to_file ON admin_to_file.admin_id = house_numbers.admin_level_id
    WHERE admin_to_file.file_id IS NOT NULL
    GROUP BY admin_to_file.file_id
  )
  UPDATE osm_pbf_files SET
    house_numbers_count = COALESCE(
      (SELECT total FROM counts_per_file WHERE counts_per_file.file_id = osm_pbf_files.id),
      0
    )
  WHERE file_path IS NOT NULL
";

pub(crate) fn update_house_numbers_count(conn: &Connection) {
  conn
    .execute(SQL_UPDATE_HOUSE_NUMBERS_COUNT, [])
    .expect("failed to update osm_pbf_files house_numbers_count");
}

#[allow(dead_code)]
const SQL_LIST_ALL: &str = "
  SELECT
    id,
    geofabrik_id, geofabrik_name, geofabrik_parent, geofabrik_url, geofabrik_wkt,
    file_path, size_bytes, md5, downloaded_at,
    osm_data_extracted_at,
    osm_header_bbox_wkt,
    osm_header_writingprogram, osm_header_source,
    osm_header_osmosis_replication_timestamp,
    osm_header_osmosis_replication_sequence_number,
    osm_header_osmosis_replication_base_url,
    node_count, way_count, relation_count,
    admin_levels_count, house_numbers_count,
    created_at, updated_at
  FROM osm_pbf_files
  ORDER BY id
";

#[allow(dead_code)]
fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<osm_pbf_files> {
  Ok(osm_pbf_files {
    id: row.get(0)?,
    geofabrik_id: row.get(1)?,
    geofabrik_name: row.get(2)?,
    geofabrik_parent: row.get(3)?,
    geofabrik_url: row.get(4)?,
    geofabrik_wkt: row.get(5)?,
    file_path: row.get(6)?,
    size_bytes: row.get(7)?,
    md5: row.get(8)?,
    downloaded_at: row.get(9)?,
    osm_data_extracted_at: row.get(10)?,
    osm_header_bbox_wkt: row.get(11)?,
    osm_header_required_features: None,
    osm_header_optional_features: None,
    osm_header_writingprogram: row.get(12)?,
    osm_header_source: row.get(13)?,
    osm_header_osmosis_replication_timestamp: row.get(14)?,
    osm_header_osmosis_replication_sequence_number: row.get(15)?,
    osm_header_osmosis_replication_base_url: row.get(16)?,
    node_count: row.get(17)?,
    way_count: row.get(18)?,
    relation_count: row.get(19)?,
    admin_levels_count: row.get(20)?,
    house_numbers_count: row.get(21)?,
    created_at: row.get(22)?,
    updated_at: row.get(23)?,
  })
}

#[allow(dead_code)]
pub(crate) fn list_all(conn: &Connection) -> Vec<osm_pbf_files> {
  conn
    .prepare(SQL_LIST_ALL)
    .expect("failed to prepare")
    .query_map([], map_row)
    .expect("failed to query")
    .collect::<Result<Vec<_>, _>>()
    .expect("failed to collect")
}
