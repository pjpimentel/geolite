use crate::database::jsonb;

// a way as the pbf file describes it: an ordered list of node ids, plus tags. the geometry is not
// here — it only exists once the referenced nodes are looked up.
pub struct osm_way {
  pub id: i64,
  pub refs: Vec<i64>,
  pub tags: std::collections::HashMap<String, String>,
}

// the storage shape of a way: the payload is already encoded.
//
// same reasoning as `osm_node_row` — the extraction pipeline builds these in its decoder threads
// and sizes its write buffer from `payload.capacity()`, so the encoding happens before the insert.
pub struct osm_way_row {
  pub id: u64,
  pub osm_pbf_chunk_id: u32,
  // pre-encoded JSONB binary (sqlite jsonb format) — bound directly as a BLOB
  pub payload: Vec<u8>,
}

impl osm_way_row {
  // the only way to build a row: from the way it stores. the encoder is passed in so its scratch
  // buffers survive across rows.
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
