use std::io::Read;

use super::message::blob_msg;

// a blob is stored either raw or zlib-compressed; the two fields are mutually exclusive and one of
// them is always set in a well-formed file.
pub fn decompress(blob: &blob_msg) -> Vec<u8> {
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
