use prost::Message;

use crate::domain::osm_pbf_file::message::blob_msg;
use crate::extract::pbf_fixtures::{self, blob_compression, make_blob};

use super::decompress;

fn decode(bytes: &[u8]) -> blob_msg {
  blob_msg::decode(bytes).expect("failed to decode blob")
}

#[test]
fn _00_returns_raw_bytes_when_blob_is_uncompressed() {
  let payload = b"conteudo do bloco".to_vec();
  let blob = decode(&make_blob(&payload, blob_compression::raw));

  assert_eq!(decompress(&blob), payload);
}

#[test]
fn _01_inflates_zlib_blob_payload() {
  // repeating payload so that zlib actually compresses it
  let payload = b"osm".repeat(500);
  let bytes = make_blob(&payload, blob_compression::zlib);
  let blob = decode(&bytes);

  assert_eq!(decompress(&blob), payload);
  assert!(
    bytes.len() < payload.len(),
    "o fixture deveria estar comprimido de fato"
  );
}

#[test]
fn _02_prefers_raw_over_zlib_when_both_are_present() {
  use std::io::Write;
  let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
  encoder.write_all(b"do zlib").expect("failed to deflate");

  let blob = decode(
    &pbf_fixtures::blob_wire {
      raw: Some(b"do raw".to_vec()),
      raw_size: Some(6),
      zlib_data: Some(encoder.finish().expect("failed to finish deflate")),
    }
    .encode_to_vec(),
  );

  assert_eq!(decompress(&blob), b"do raw");
}

#[test]
#[should_panic(expected = "unsupported blob compression")]
fn _03_panics_when_blob_has_neither_raw_nor_zlib() {
  let blob = decode(&make_blob(b"ignorado", blob_compression::none));
  decompress(&blob);
}
