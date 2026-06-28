use prost::Message;
use std::{
  fs,
  io::{Read, Seek},
};

#[derive(prost::Message)]
struct header_block_msg {
  #[prost(message, optional, tag = "1")]
  bbox: Option<header_bbox_msg>,
  #[prost(string, repeated, tag = "4")]
  required_features: Vec<String>,
  #[prost(string, repeated, tag = "5")]
  optional_features: Vec<String>,
  #[prost(string, optional, tag = "16")]
  writingprogram: Option<String>,
  #[prost(string, optional, tag = "17")]
  source: Option<String>,
  #[prost(int64, optional, tag = "32")]
  osmosis_replication_timestamp: Option<i64>,
  #[prost(int64, optional, tag = "33")]
  osmosis_replication_sequence_number: Option<i64>,
  #[prost(string, optional, tag = "34")]
  osmosis_replication_base_url: Option<String>,
}

#[derive(prost::Message)]
struct header_bbox_msg {
  #[prost(sint64, tag = "1")]
  left: i64,
  #[prost(sint64, tag = "2")]
  right: i64,
  #[prost(sint64, tag = "3")]
  top: i64,
  #[prost(sint64, tag = "4")]
  bottom: i64,
}

pub struct header_bbox {
  pub bottom: f64,
  pub left: f64,
  pub top: f64,
  pub right: f64,
}

pub struct header_output {
  pub bbox: Option<header_bbox>,
  pub writingprogram: Option<String>,
  pub source: Option<String>,
  pub replication_timestamp: Option<i64>,
}

pub fn run(pbf: &str, conn: &rusqlite::Connection, file_id: u32) -> header_output {
  let chunk = crate::database::osm_pbf_blob_chunks::get_header_chunk(conn, file_id)
    .expect("no header chunk found in sqlite — run extract osm-pbf-blob-chunks first");

  let mut file = fs::File::open(pbf).expect("failed to open pbf file");
  file
    .seek(std::io::SeekFrom::Start(chunk.data_first_byte))
    .expect("failed to seek to header blob");

  let mut blob_buf = vec![0u8; chunk.data_size as usize];
  file
    .read_exact(&mut blob_buf)
    .expect("failed to read header blob");

  let blob = super::osm_data::blob_msg::decode(blob_buf.as_slice()).expect("failed to decode blob");
  let raw = super::decompress_blob(&blob);

  let h = header_block_msg::decode(raw.as_slice()).expect("failed to decode header block");

  const NANO: f64 = 1e-9;

  let bbox_wkt = h.bbox.as_ref().map(|b| {
    let left = b.left as f64 * NANO;
    let right = b.right as f64 * NANO;
    let top = b.top as f64 * NANO;
    let bot = b.bottom as f64 * NANO;
    format!("POLYGON(({left} {bot},{right} {bot},{right} {top},{left} {top},{left} {bot}))")
      .into_bytes()
  });

  let required_features = if h.required_features.is_empty() {
    None
  } else {
    Some(serde_json::to_vec(&h.required_features).expect("failed to serialize required_features"))
  };

  let optional_features = if h.optional_features.is_empty() {
    None
  } else {
    Some(serde_json::to_vec(&h.optional_features).expect("failed to serialize optional_features"))
  };

  crate::database::osm_pbf_files::update_osm_header(
    conn,
    pbf,
    bbox_wkt,
    required_features,
    optional_features,
    h.writingprogram.as_deref(),
    h.source.as_deref(),
    h.osmosis_replication_timestamp,
    h.osmosis_replication_sequence_number
      .and_then(|v| u32::try_from(v).ok()),
    h.osmosis_replication_base_url.as_deref(),
  );

  header_output {
    bbox: h.bbox.map(|b| header_bbox {
      bottom: b.bottom as f64 * NANO,
      left: b.left as f64 * NANO,
      top: b.top as f64 * NANO,
      right: b.right as f64 * NANO,
    }),
    writingprogram: h.writingprogram,
    source: h.source,
    replication_timestamp: h.osmosis_replication_timestamp,
  }
}
