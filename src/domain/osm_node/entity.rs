use crate::database::jsonb;

pub struct osm_node {
  pub id: i64,
  pub lat: f64,
  pub lon: f64,
  pub tags: std::collections::HashMap<String, String>,
}

// encoded up front, in the decoder threads, and public for that reason: the writer sizes its
// buffer from `payload.capacity()`. taking an `osm_node` at insert time would move the encoding
// into the single writer thread and serialise what is parallel today.
pub struct osm_node_row {
  pub id: u64,
  pub osm_pbf_chunk_id: u32,
  pub payload: Vec<u8>,
}

impl osm_node_row {
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
