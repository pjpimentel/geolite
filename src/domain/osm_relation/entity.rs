use crate::database::jsonb;

pub struct osm_relation {
  pub id: i64,
  pub tags: std::collections::HashMap<String, String>,
  pub members: Vec<osm_relation_member>,
}

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

// encoded up front in the decoder threads, like `osm_node_row` — moving the encoding to insert
// time would serialise the pipeline.
pub struct osm_relation_row {
  pub id: u64,
  pub osm_pbf_chunk_id: u32,
  pub payload: Vec<u8>,
}

impl osm_relation_row {
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
