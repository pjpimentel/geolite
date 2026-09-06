use super::*;

use crate::extract::pbf_fixtures::{self, blob_compression, header_blob, header_spec};

struct scene {
  temp: pbf_fixtures::temp_scene,
  file_id: u32,
  conn: rusqlite::Connection,
}

fn setup(tag: &str, spec: &header_spec, compression: blob_compression) -> scene {
  let chunk = pbf_fixtures::make_chunk("OSMHeader", &header_blob(spec, compression));
  let temp = pbf_fixtures::temp_scene(tag);
  pbf_fixtures::write_pbf(&temp.pbf_path, &[chunk]);

  let conn = crate::database::open_write(&temp.db_path);
  let file_id = crate::domain::osm_pbf_file::repository::ensure_by_file_path(&conn, &temp.pbf_path);
  super::super::blob_scanner::run(&temp.pbf_path, &conn, file_id, |_| {});

  scene {
    temp,
    file_id,
    conn,
  }
}

fn blob_column_as_text(s: &scene, name: &str) -> Option<String> {
  column::<Vec<u8>>(s, name)
    .map(|bytes| String::from_utf8(bytes).expect("coluna deve conter utf-8 valido"))
}

fn column<T: rusqlite::types::FromSql>(s: &scene, name: &str) -> Option<T> {
  s.conn
    .query_row(
      &format!("SELECT {name} FROM osm_pbf_files WHERE id = ?1"),
      rusqlite::params![s.file_id],
      |row| row.get::<_, Option<T>>(0),
    )
    .expect("failed to read osm_pbf_files")
}

#[test]
fn _00_returns_bbox_converted_from_nanodegrees() {
  let s = setup(
    "hd_00_00",
    &header_spec::default(),
    blob_compression::zlib,
  );
  let out = run(&s.temp.pbf_path, &s.conn, s.file_id);

  let bbox = out.bbox.expect("o fixture define uma bbox");
  assert!((bbox.left - -9.5).abs() < 1e-9);
  assert!((bbox.right - -9.0).abs() < 1e-9);
  assert!((bbox.top - 38.75).abs() < 1e-9);
  assert!((bbox.bottom - 38.5).abs() < 1e-9);
}

#[test]
fn _01_writes_bbox_polygon_wkt_to_osm_pbf_files() {
  let s = setup(
    "hd_00_01",
    &header_spec::default(),
    blob_compression::zlib,
  );
  run(&s.temp.pbf_path, &s.conn, s.file_id);

  let wkt = blob_column_as_text(&s, "osm_header_bbox_wkt").expect("wkt deve estar preenchido");
  assert_eq!(
    wkt,
    "POLYGON((-9.5 38.5,-9 38.5,-9 38.75,-9.5 38.75,-9.5 38.5))"
  );
}

#[test]
fn _02_returns_none_bbox_when_header_has_no_bbox() {
  let s = setup(
    "hd_00_02",
    &header_spec {
      bbox: None,
      ..Default::default()
    },
    blob_compression::zlib,
  );
  let out = run(&s.temp.pbf_path, &s.conn, s.file_id);

  assert!(out.bbox.is_none());
  assert_eq!(column::<String>(&s, "osm_header_bbox_wkt"), None);
}

#[test]
fn _03_serializes_required_and_optional_features_as_json() {
  let s = setup(
    "hd_00_03",
    &header_spec::default(),
    blob_compression::zlib,
  );
  run(&s.temp.pbf_path, &s.conn, s.file_id);

  let required = blob_column_as_text(&s, "osm_header_required_features")
    .expect("required deve estar preenchido");
  let optional = blob_column_as_text(&s, "osm_header_optional_features")
    .expect("optional deve estar preenchido");

  assert_eq!(required, r#"["OsmSchema-V0.6","DenseNodes"]"#);
  assert_eq!(optional, r#"["Has_Metadata"]"#);
}

#[test]
fn _04_stores_null_features_when_lists_are_empty() {
  let s = setup(
    "hd_00_04",
    &header_spec {
      required_features: Vec::new(),
      optional_features: Vec::new(),
      ..Default::default()
    },
    blob_compression::zlib,
  );
  run(&s.temp.pbf_path, &s.conn, s.file_id);

  assert_eq!(column::<String>(&s, "osm_header_required_features"), None);
  assert_eq!(column::<String>(&s, "osm_header_optional_features"), None);
}

#[test]
fn _05_propagates_writingprogram_source_and_osmosis_fields() {
  let s = setup(
    "hd_00_05",
    &header_spec::default(),
    blob_compression::zlib,
  );
  let out = run(&s.temp.pbf_path, &s.conn, s.file_id);

  assert_eq!(out.writingprogram.as_deref(), Some("geolite-test"));
  assert_eq!(out.source.as_deref(), Some("fixture"));
  assert_eq!(out.replication_timestamp, Some(1_700_000_000));

  assert_eq!(
    column::<String>(&s, "osm_header_writingprogram").as_deref(),
    Some("geolite-test")
  );
  assert_eq!(
    column::<String>(&s, "osm_header_source").as_deref(),
    Some("fixture")
  );
  assert_eq!(
    column::<i64>(&s, "osm_header_osmosis_replication_timestamp"),
    Some(1_700_000_000)
  );
  assert_eq!(
    column::<i64>(&s, "osm_header_osmosis_replication_sequence_number"),
    Some(4_242)
  );
  assert_eq!(
    column::<String>(&s, "osm_header_osmosis_replication_base_url").as_deref(),
    Some("https://example.invalid/replication")
  );
}

#[test]
fn _06_drops_negative_replication_sequence_number() {
  let s = setup(
    "hd_00_06",
    &header_spec {
      replication_sequence_number: Some(-1),
      ..Default::default()
    },
    blob_compression::zlib,
  );
  run(&s.temp.pbf_path, &s.conn, s.file_id);

  assert_eq!(
    column::<i64>(&s, "osm_header_osmosis_replication_sequence_number"),
    None,
    "valor fora do range de u32 deve ser descartado"
  );
}

#[test]
fn _07_decodes_uncompressed_header_blob() {
  let s = setup("hd_00_07", &header_spec::default(), blob_compression::raw);
  let out = run(&s.temp.pbf_path, &s.conn, s.file_id);

  assert_eq!(out.writingprogram.as_deref(), Some("geolite-test"));
  assert!(out.bbox.is_some());
}

#[test]
#[should_panic(expected = "no header chunk found")]
fn _08_panics_when_no_header_chunk_is_indexed() {
  let temp = pbf_fixtures::temp_scene("hd_00_08");
  pbf_fixtures::write_pbf(&temp.pbf_path, &[]);

  let conn = crate::database::open_write(&temp.db_path);
  let file_id = crate::domain::osm_pbf_file::repository::ensure_by_file_path(&conn, &temp.pbf_path);

  run(&temp.pbf_path, &conn, file_id);
}
