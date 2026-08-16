use crate::database::jsonb;

// a relation as the pbf file describes it: an ordered list of members, each pointing at a node, a
// way or another relation and carrying a role. administrative boundaries are relations whose way
// members, joined end to end, close into rings.
pub struct osm_relation {
  pub id: i64,
  pub tags: std::collections::HashMap<String, String>,
  pub members: Vec<osm_relation_member>,
}

// the discriminants match the pbf `types` field, which is what the decoder reads.
#[derive(Clone, Copy)]
pub enum osm_member_type {
  node = 0,
  way = 1,
  relation = 2,
}

pub struct osm_relation_member {
  pub osm_member_type: osm_member_type,
  pub id: i64,
  pub role: String,
}

// the storage shape of a relation: the payload is already encoded.
//
// same reasoning as `osm_node_row` and `osm_way_row` — the pipeline builds these in its decoder
// threads and sizes its write buffer from `payload.capacity()`.
pub struct osm_relation_row {
  pub id: u64,
  pub osm_pbf_chunk_id: u32,
  // pre-encoded JSONB binary (sqlite jsonb format) — bound directly as a BLOB
  pub payload: Vec<u8>,
}

impl osm_relation_row {
  // the only way to build a row: from the relation it stores.
  pub fn encode(
    relation: &osm_relation,
    osm_pbf_chunk_id: u32,
    encoder: &mut jsonb::encoder,
  ) -> Self {
    let mut payload = Vec::with_capacity(128 + relation.members.len() * 32);
    super::payload::encode(encoder, &mut payload, relation);
    Self {
      id: relation.id as u64,
      osm_pbf_chunk_id,
      payload,
    }
  }
}
