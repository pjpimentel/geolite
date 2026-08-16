use crate::database::jsonb;

// a node as the pbf file describes it: a point with tags. the id is the osm node id.
pub struct osm_node {
  pub id: i64,
  pub lat: f64,
  pub lon: f64,
  pub tags: std::collections::HashMap<String, String>,
}

// the storage shape of a node: the payload is already encoded.
//
// it is public, and encoded up front, on purpose. the extraction pipeline builds these in its
// decoder threads — several at once — and sizes its write buffer from `payload.capacity()`. taking
// an `osm_node` at insert time instead would move the encoding into the single writer thread and
// serialise what is parallel today.
pub struct osm_node_row {
  pub id: u64,
  pub osm_pbf_chunk_id: u32,
  // pre-encoded JSONB binary (sqlite jsonb format) — bound directly as a BLOB
  pub payload: Vec<u8>,
}

impl osm_node_row {
  // the only way to build a row: from the node it stores. the encoder is passed in so its scratch
  // buffers survive across rows.
  pub fn encode(node: &osm_node, osm_pbf_chunk_id: u32, encoder: &mut jsonb::encoder) -> Self {
    let mut payload = Vec::with_capacity(128);
    super::payload::encode(encoder, &mut payload, node);
    Self {
      id: node.id as u64,
      osm_pbf_chunk_id,
      payload,
    }
  }
}
