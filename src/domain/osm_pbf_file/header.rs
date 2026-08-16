// reads the file's first blob — the osm header — and writes what it says into the file's row: the
// bounding box it covers, the features it needs, the program that wrote it and where its
// replication stream lives.

use prost::Message;
use std::{
  fs,
  io::{Read, Seek},
};

use super::{blob_index, repository};
use crate::pbf::{blob, message};

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
  let chunk = blob_index::get_header_chunk(conn, file_id)
    .expect("no header chunk found in sqlite — run extract osm-pbf-blob-chunks first");

  let mut file = fs::File::open(pbf).expect("failed to open pbf file");
  file
    .seek(std::io::SeekFrom::Start(chunk.data_first_byte))
    .expect("failed to seek to header blob");

  let mut blob_buf = vec![0u8; chunk.data_size as usize];
  file
    .read_exact(&mut blob_buf)
    .expect("failed to read header blob");

  let blob = message::blob_msg::decode(blob_buf.as_slice()).expect("failed to decode blob");
  let raw = blob::decompress(&blob);

  let h = message::header_block_msg::decode(raw.as_slice()).expect("failed to decode header block");

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

  repository::update_osm_header(
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

#[cfg(test)]
#[path = "header.test.rs"]
mod tests;
