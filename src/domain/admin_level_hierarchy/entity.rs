// the two shapes of a hierarchy row and the storage format of the ancestor chain.
//
// the chain is written as a json array of admin level ids ordered from the most specific enclosing
// area to the most general — `[bairro, cidade, estado, país]` — and stored as jsonb, so the write
// side encodes to text and lets sqlite's `jsonb()` do the rest, and the read side asks for
// `json(...)` back.

// the write shape: what the resolver produces and the repository inserts.
pub struct hierarchy_row {
  pub admin_level_id: i64,
  pub ancestor_ids: String,
  pub user_friendly_name: String,
}

// the read shape: what the query path and the search index consume.
pub struct hierarchy_lookup_row {
  pub user_friendly_name: String,
  pub ancestor_ids: Vec<i64>,
}

pub fn encode_chain(ids: &[i64]) -> String {
  serde_json::to_string(ids).unwrap_or_else(|_| "[]".to_string())
}

pub fn decode_chain(json: &str) -> Vec<i64> {
  serde_json::from_str(json).unwrap_or_default()
}
