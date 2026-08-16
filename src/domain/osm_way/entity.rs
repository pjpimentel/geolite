use crate::database::jsonb;

pub struct osm_way {
  pub id: i64,
  pub refs: Vec<i64>,
  pub tags: std::collections::HashMap<String, String>,
}

// encoded up front in the decoder threads, like `osm_node_row` — moving the encoding to insert
// time would serialise the pipeline.
pub struct osm_way_row {
  pub id: u64,
  pub osm_pbf_chunk_id: u32,
  pub payload: Vec<u8>,
}

impl osm_way_row {
  pub fn encode(way: &osm_way, osm_pbf_chunk_id: u32, encoder: &mut jsonb::encoder) -> Self {
    let mut payload = Vec::with_capacity(128 + way.refs.len() * 4);
    super::payload::encode(encoder, &mut payload, way);
    Self {
      id: way.id as u64,
      osm_pbf_chunk_id,
      payload,
    }
  }
}
