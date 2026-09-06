#[derive(serde::Serialize, serde::Deserialize)]
pub struct osm_way {
  #[serde(skip_serializing, default)]
  pub id: i64,
  pub refs: Vec<i64>,
  pub tags: std::collections::HashMap<String, String>,
}
