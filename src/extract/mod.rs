pub mod admin_levels;
pub mod blob_chunks;
pub mod header;
pub mod house_numbers;
pub mod osm_data;

fn decompress_blob(blob: &osm_data::blob_msg) -> Vec<u8> {
  use std::io::Read;
  let raw = blob.raw();
  if !raw.is_empty() {
    return raw.to_vec();
  }
  let zlib = blob.zlib_data();
  if !zlib.is_empty() {
    let mut decoder = flate2::read::ZlibDecoder::new(zlib);
    let mut buf = Vec::new();
    decoder
      .read_to_end(&mut buf)
      .expect("failed to decompress blob");
    return buf;
  }
  panic!("unsupported blob compression");
}
