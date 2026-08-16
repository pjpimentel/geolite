// walks a `.osm.pbf` file front to back, reading only the length-prefixed blob headers and skipping
// the blob bodies, to record where every blob starts and ends. it is what fills `blob_index`, and it
// is the only pass over the file that never decompresses anything.

use prost::Message;
use std::{
  fs,
  io::{self, Read},
};

use super::blob_index::{self, chunk_type, osm_pbf_blob_chunk};
use crate::pbf::message::blob_header_msg;

pub(crate) struct progress {
  pub(crate) total_bytes: u64,
  pub(crate) bytes_read: u64,
}

pub fn run(
  pbf: &str,
  conn: &rusqlite::Connection,
  file_id: u32,
  on_progress: impl Fn(progress),
) -> usize {
  let file = fs::File::open(pbf).expect("failed to open pbf file");
  let total_bytes = file.metadata().expect("failed to read file metadata").len();
  let mut reader = io::BufReader::new(file);
  let mut offset: u64 = 0;
  let mut chunks: Vec<osm_pbf_blob_chunk> = Vec::new();

  loop {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
      Ok(_) => {}
      Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
      Err(e) => panic!("read error: {e}"),
    }
    let header_len = u32::from_be_bytes(len_buf) as usize;

    let mut header_buf = vec![0u8; header_len];
    reader
      .read_exact(&mut header_buf)
      .expect("failed to read blob header");

    let bh = blob_header_msg::decode(header_buf.as_slice()).expect("failed to decode blob header");

    let blob_size = bh.datasize as u64;
    let first_byte = offset;
    let chunk_size = 4 + header_len as u64 + blob_size;
    let data_first_byte = offset + 4 + header_len as u64;
    let chunk_type = match bh.r#type.as_str() {
      "OSMHeader" => chunk_type::header,
      _ => chunk_type::data,
    };

    chunks.push(osm_pbf_blob_chunk {
      id: 0,
      file_id,
      first_byte,
      chunk_size,
      data_first_byte,
      data_size: blob_size,
      chunk_type,
    });

    io::copy(&mut reader.by_ref().take(blob_size), &mut io::sink())
      .expect("failed to skip blob data");

    offset += chunk_size;
    on_progress(progress {
      total_bytes,
      bytes_read: offset,
    });
  }

  blob_index::batch_insert(conn, &chunks);
  chunks.len()
}
