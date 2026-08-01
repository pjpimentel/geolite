use super::*;

use crate::extract::pbf_fixtures::{self, blob_compression, header_blob, header_spec};

struct scene {
  temp: pbf_fixtures::temp_scene,
  file_id: u32,
  conn: rusqlite::Connection,
}

// escreve um pbf contendo apenas o blob de header informado e indexa os chunks
fn setup(tag: &str, spec: &header_spec, compression: blob_compression) -> scene {
  let chunk = pbf_fixtures::make_chunk("OSMHeader", &header_blob(spec, compression));
  let temp = pbf_fixtures::temp_scene(tag);
  pbf_fixtures::write_pbf(&temp.pbf_path, &[chunk]);

  let conn = crate::database::open_write(&temp.db_path);
  let file_id = crate::database::osm_pbf_files::ensure_by_file_path(&conn, &temp.pbf_path);
  super::super::blob_chunks::run(&temp.pbf_path, &conn, file_id, |_| {});

  scene {
    temp,
    file_id,
    conn,
  }
}

// as colunas de wkt e de features sao gravadas como blob (Vec<u8>), nao como texto
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

// 00.00: bbox e convertida de nanograus para graus nos quatro cantos
#[test]
fn _00_00_returns_bbox_converted_from_nanodegrees() {
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

// 00.01: a bbox e gravada como WKT de poligono fechado, no sentido
// (left bottom, right bottom, right top, left top, left bottom)
#[test]
fn _00_01_writes_bbox_polygon_wkt_to_osm_pbf_files() {
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

// 00.02: header sem bbox devolve None e nao grava wkt
#[test]
fn _00_02_returns_none_bbox_when_header_has_no_bbox() {
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

// 00.03: listas de features sao serializadas como array json
#[test]
fn _00_03_serializes_required_and_optional_features_as_json() {
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

// 00.04: listas de features vazias viram NULL em vez de "[]"
#[test]
fn _00_04_stores_null_features_when_lists_are_empty() {
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

// 00.05: writingprogram, source e os campos osmosis chegam ao retorno e ao banco
#[test]
fn _00_05_propagates_writingprogram_source_and_osmosis_fields() {
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

// 00.06: sequence number negativo nao cabe em u32 e vira NULL
#[test]
fn _00_06_drops_negative_replication_sequence_number() {
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

// 00.07: blob sem compressao decodifica igual ao blob zlib
#[test]
fn _00_07_decodes_uncompressed_header_blob() {
  let s = setup("hd_00_07", &header_spec::default(), blob_compression::raw);
  let out = run(&s.temp.pbf_path, &s.conn, s.file_id);

  assert_eq!(out.writingprogram.as_deref(), Some("geolite-test"));
  assert!(out.bbox.is_some());
}

// 00.08: sem chunk de header indexado nao ha de onde ler o cabecalho
#[test]
#[should_panic(expected = "no header chunk found")]
fn _00_08_panics_when_no_header_chunk_is_indexed() {
  let temp = pbf_fixtures::temp_scene("hd_00_08");
  pbf_fixtures::write_pbf(&temp.pbf_path, &[]);

  let conn = crate::database::open_write(&temp.db_path);
  let file_id = crate::database::osm_pbf_files::ensure_by_file_path(&conn, &temp.pbf_path);

  run(&temp.pbf_path, &conn, file_id);
}
