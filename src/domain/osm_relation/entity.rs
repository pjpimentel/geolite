#[derive(serde::Serialize, serde::Deserialize)]
pub struct osm_relation {
  #[serde(skip_serializing, default)]
  pub id: i64,
  pub tags: std::collections::HashMap<String, String>,
  pub members: Vec<osm_relation_member>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum osm_member_type {
  #[serde(rename = "n")]
  node = 0,
  #[serde(rename = "w")]
  way = 1,
  #[serde(rename = "r")]
  relation = 2,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct osm_relation_member {
  #[serde(rename = "type")]
  pub osm_member_type: osm_member_type,
  pub id: i64,
  pub role: String,
}
