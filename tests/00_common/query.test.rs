#![allow(dead_code)]

use serde_json::Value;

pub fn matches(result: &Value) -> &Vec<Value> {
  result["matches"]
    .as_array()
    .expect("matches must be an array")
}

pub fn first(result: &Value) -> &Value {
  matches(result)
    .first()
    .unwrap_or_else(|| panic!("expected at least one match in {result}"))
}

pub fn levels_of(m: &Value) -> Vec<u64> {
  m["admin_levels"]
    .as_array()
    .expect("admin_levels must be an array")
    .iter()
    .map(|a| a["level"].as_u64().expect("level must be a number"))
    .collect()
}

pub fn name_at(m: &Value, level: u64) -> Option<String> {
  m["admin_levels"]
    .as_array()?
    .iter()
    .find(|a| a["level"].as_u64() == Some(level))
    .and_then(|a| a["name"].as_str())
    .map(|s| s.to_string())
}

pub fn level_at(m: &Value, level: u64) -> Option<&Value> {
  m["admin_levels"]
    .as_array()?
    .iter()
    .find(|a| a["level"].as_u64() == Some(level))
}

// the street ids behind a result set, sorted and deduped: segments of the same street tie on score,
// so their order follows the tantivy segment layout and must never be asserted.
pub fn way_ids(result: &Value) -> Vec<u64> {
  let mut ids: Vec<u64> = matches(result)
    .iter()
    .filter_map(|m| level_at(m, 12).and_then(|a| a["osm_way_id"].as_u64()))
    .collect();
  ids.sort_unstable();
  ids.dedup();
  ids
}
