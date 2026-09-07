#[derive(serde::Serialize, serde::Deserialize)]
pub struct osm_node {
  #[serde(skip_serializing, default)]
  pub id: i64,
  pub lat: f64,
  pub lon: f64,
  pub tags: std::collections::HashMap<String, String>,
}
