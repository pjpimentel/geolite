pub struct hierarchy_row {
  pub admin_level_id: i64,
  pub ancestor_ids: String,
  pub user_friendly_name: String,
}

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

#[cfg(test)]
#[path = "entity.test.rs"]
mod tests;
