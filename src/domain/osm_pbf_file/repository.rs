use rusqlite::Connection;

use crate::domain::table;

const SQL_CREATE: &str = "
  CREATE TABLE IF NOT EXISTS osm_pbf_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    origin INTEGER NOT NULL,
    origin_id VARCHAR(128),
    origin_name VARCHAR(128),
    url VARCHAR(512),
    origin_wkt BLOB,
    path VARCHAR(1024) UNIQUE,
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
    updated_at INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE (origin, origin_id)
  );
  CREATE TRIGGER IF NOT EXISTS osm_pbf_files_updated_at
    AFTER UPDATE ON osm_pbf_files
    FOR EACH ROW
    BEGIN
      UPDATE osm_pbf_files SET updated_at = unixepoch() WHERE id = OLD.id;
    END;
";

const SQL_CREATE_INDEXES: &str = "
  CREATE INDEX IF NOT EXISTS osm_pbf_files_search_by_url
    ON osm_pbf_files(url);
";

pub struct osm_pbf_files;

impl table for osm_pbf_files {
  const CREATE: &str = SQL_CREATE;
  const INDEXES: &str = SQL_CREATE_INDEXES;
}

pub(crate) fn add_origin_wkt(conn: &Connection) {
  const SQL_ADD_ORIGIN_WKT: &str = "ALTER TABLE osm_pbf_files ADD COLUMN origin_wkt BLOB";

  if crate::database::has_column(conn, "osm_pbf_files", "origin_wkt") {
    return;
  }
  conn
    .execute_batch(SQL_ADD_ORIGIN_WKT)
    .expect("failed to add origin_wkt to osm_pbf_files");
}

pub enum origin {
  local_path(String),
  geofabrik { id: String, url: String },
  url(String),
}

impl origin {
  pub fn code(&self) -> u8 {
    match self {
      origin::local_path(_) => 0,
      origin::geofabrik { .. } => 1,
      origin::url(_) => 2,
    }
  }

  pub fn download_url(&self) -> Option<&str> {
    match self {
      origin::local_path(_) => None,
      origin::geofabrik { url, .. } => Some(url),
      origin::url(url) => Some(url),
    }
  }
}

fn file_name_of(path_or_url: &str) -> &str {
  std::path::Path::new(path_or_url)
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or(path_or_url)
}

pub fn upsert_geofabrik_index_item(
  conn: &Connection,
  geofabrik_id: &str,
  name: &str,
  url: &str,
  wkt: Option<&str>,
) {
  const SQL_PROMOTE_URL_TO_GEOFABRIK: &str = "
    UPDATE OR IGNORE osm_pbf_files SET
      origin = 1,
      origin_id = ?1,
      origin_name = ?2,
      origin_wkt = ?4
    WHERE url = ?3
      AND origin = 2
  ";

  const SQL_UPSERT_GEOFABRIK: &str = "
    INSERT INTO osm_pbf_files (
      origin,
      origin_id,
      origin_name,
      url,
      origin_wkt
    ) VALUES (
      1,
      ?1,
      ?2,
      ?3,
      ?4
    )
    ON CONFLICT(origin, origin_id) DO UPDATE SET
      origin_name = excluded.origin_name,
      url = excluded.url,
      origin_wkt = excluded.origin_wkt
  ";

  let coverage = wkt.map(str::as_bytes);
  let promoted = conn
    .execute(
      SQL_PROMOTE_URL_TO_GEOFABRIK,
      rusqlite::params![geofabrik_id, name, url, coverage],
    )
    .expect("failed to promote url row to geofabrik");
  if promoted == 0 {
    conn
      .execute(
        SQL_UPSERT_GEOFABRIK,
        rusqlite::params![geofabrik_id, name, url, coverage],
      )
      .expect("failed to upsert geofabrik index item");
  }
}

pub fn list_geofabrik_index(conn: &Connection) -> Vec<(String, String, String)> {
  const SQL_LIST_GEOFABRIK_INDEX: &str = "
    SELECT
      origin_id,
      origin_name,
      url
    FROM osm_pbf_files
    WHERE origin = 1
    ORDER BY origin_id
  ";

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

pub fn get_geofabrik_url(conn: &Connection, geofabrik_id: &str) -> Option<String> {
  const SQL_GET_GEOFABRIK_URL_BY_ID: &str = "
    SELECT url
    FROM osm_pbf_files
    WHERE origin = 1
      AND origin_id = ?1
    LIMIT 1
  ";

  conn
    .query_row(
      SQL_GET_GEOFABRIK_URL_BY_ID,
      rusqlite::params![geofabrik_id],
      |row| row.get::<_, Option<String>>(0),
    )
    .ok()
    .flatten()
}

const SQL_GET_ID_BY_PATH: &str = "SELECT id FROM osm_pbf_files WHERE path = ?1";

pub fn get_id_by_path(conn: &Connection, file_path: &str) -> Option<u32> {
  conn
    .query_row(SQL_GET_ID_BY_PATH, rusqlite::params![file_path], |row| {
      row.get(0)
    })
    .ok()
}

pub fn ensure_by_file_path(conn: &Connection, file_path: &str) -> u32 {
  const SQL_ENSURE_LOCAL_PATH: &str = "
    INSERT OR IGNORE INTO osm_pbf_files (
      origin,
      origin_name,
      path
    ) VALUES (
      ?1,
      ?2,
      ?3
    )
  ";

  let from = origin::local_path(file_path.to_string());
  conn
    .execute(
      SQL_ENSURE_LOCAL_PATH,
      rusqlite::params![from.code(), file_name_of(file_path), file_path],
    )
    .expect("failed to ensure osm_pbf_files row");
  conn
    .query_row(SQL_GET_ID_BY_PATH, rusqlite::params![file_path], |row| {
      row.get(0)
    })
    .expect("failed to get id after ensure")
}

pub fn clear_download(conn: &Connection, file_path: &str) {
  const SQL_CLEAR_DOWNLOAD: &str = "
    UPDATE osm_pbf_files SET
      path = NULL,
      size_bytes = NULL,
      md5 = NULL,
      downloaded_at = NULL
    WHERE path = ?1
  ";

  conn
    .execute(SQL_CLEAR_DOWNLOAD, rusqlite::params![file_path])
    .expect("failed to clear download");
}

pub fn get_file_path(conn: &Connection, id_or_geofabrik_id: &str) -> Option<String> {
  const SQL_GET_FILE_PATH: &str = "
    SELECT path
    FROM osm_pbf_files
    WHERE (origin_id = ?1 OR url = ?1 OR CAST(id AS TEXT) = ?1)
      AND path IS NOT NULL
    LIMIT 1
  ";

  conn
    .query_row(
      SQL_GET_FILE_PATH,
      rusqlite::params![id_or_geofabrik_id],
      |row| row.get(0),
    )
    .ok()
    .flatten()
}

#[allow(clippy::too_many_arguments)]
pub fn update_osm_header(
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
    WHERE path = ?9
  ";

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

pub fn update_counts(
  conn: &Connection,
  file_id: u32,
  node_count: u64,
  way_count: u64,
  relation_count: u64,
) {
  const SQL_UPDATE_COUNTS: &str = "
    UPDATE osm_pbf_files SET
      node_count = ?1,
      way_count = ?2,
      relation_count = ?3,
      osm_data_extracted_at = UNIXEPOCH()
    WHERE id = ?4
  ";

  conn
    .execute(
      SQL_UPDATE_COUNTS,
      rusqlite::params![node_count, way_count, relation_count, file_id],
    )
    .expect("failed to update osm_pbf_files counts");
}

pub fn update_downloaded(
  conn: &Connection,
  from: &origin,
  file_path: &str,
  size_bytes: u64,
  md5: &str,
) {
  const SQL_UPDATE_DOWNLOADED_GEOFABRIK: &str = "
    UPDATE osm_pbf_files SET
      path = ?1,
      size_bytes = ?2,
      md5 = ?3,
      downloaded_at = UNIXEPOCH()
    WHERE origin = 1
      AND origin_id = ?4
  ";

  const SQL_UPDATE_DOWNLOADED_URL: &str = "
    UPDATE osm_pbf_files SET
      path = ?1,
      size_bytes = ?2,
      md5 = ?3,
      downloaded_at = UNIXEPOCH()
    WHERE url = ?4
  ";

  const SQL_INSERT_DOWNLOADED: &str = "
    INSERT INTO osm_pbf_files (
      origin,
      origin_id,
      origin_name,
      url,
      path,
      size_bytes,
      md5,
      downloaded_at
    ) VALUES (
      ?1,
      ?2,
      ?3,
      ?4,
      ?5,
      ?6,
      ?7,
      UNIXEPOCH()
    )
    ON CONFLICT(path) DO UPDATE SET
      origin = excluded.origin,
      origin_id = excluded.origin_id,
      origin_name = excluded.origin_name,
      url = excluded.url,
      size_bytes = excluded.size_bytes,
      md5 = excluded.md5,
      downloaded_at = excluded.downloaded_at
  ";

  let (update, key, origin_id, url) = match from {
    origin::geofabrik { id, url } => (SQL_UPDATE_DOWNLOADED_GEOFABRIK, id.as_str(), Some(id.as_str()), url.as_str()),
    origin::url(url) => (SQL_UPDATE_DOWNLOADED_URL, url.as_str(), None, url.as_str()),
    origin::local_path(path) => panic!("a local file is never recorded as downloaded: {path}"),
  };
  let affected = conn
    .execute(update, rusqlite::params![file_path, size_bytes, md5, key])
    .expect("failed to update downloaded");
  if affected == 0 {
    conn
      .execute(
        SQL_INSERT_DOWNLOADED,
        rusqlite::params![from.code(), origin_id, file_name_of(url), url, file_path, size_bytes, md5],
      )
      .expect("failed to insert downloaded");
  }
}

pub fn update_admin_levels_count(conn: &Connection) {
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
    WHERE path IS NOT NULL
  ";

  conn
    .execute(SQL_UPDATE_ADMIN_LEVELS_COUNT, [])
    .expect("failed to update osm_pbf_files admin_levels_count");
}

pub fn update_house_numbers_count(conn: &Connection) {
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
    WHERE path IS NOT NULL
  ";

  conn
    .execute(SQL_UPDATE_HOUSE_NUMBERS_COUNT, [])
    .expect("failed to update osm_pbf_files house_numbers_count");
}

#[cfg(test)]
#[path = "repository.test.rs"]
mod tests;
