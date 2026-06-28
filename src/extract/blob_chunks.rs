use prost::Message;
use std::{
  fs,
  io::{self, Read},
};

#[derive(prost::Message)]
struct blob_header_msg {
  #[prost(string, tag = "1")]
  pub(super) r#type: String,
  // wire-compatible with int32 for non-negative values; datasize is always >= 0 in practice
  #[prost(uint32, tag = "3")]
  pub(super) datasize: u32,
}

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
  let mut chunks: Vec<crate::database::osm_pbf_blob_chunks::osm_pbf_blob_chunk> = Vec::new();

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
      "OSMHeader" => crate::database::osm_pbf_blob_chunks::chunk_type::header,
      _ => crate::database::osm_pbf_blob_chunks::chunk_type::data,
    };

    chunks.push(crate::database::osm_pbf_blob_chunks::osm_pbf_blob_chunk {
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

  crate::database::osm_pbf_blob_chunks::batch_insert(conn, &chunks);
  chunks.len()
}
