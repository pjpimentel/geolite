#![allow(dead_code)]

use super::harness::world;
use serde_json::Value;

pub fn round(value: &Value, decimals: i32) -> Value {
  match value.as_f64() {
    Some(f) => {
      let factor = 10f64.powi(decimals);
      serde_json::json!((f * factor).round() / factor)
    }
    None => value.clone(),
  }
}

pub fn normalize(value: &Value) -> Value {
  match value {
    Value::Object(map) => {
      let mut out = serde_json::Map::new();
      for (k, v) in map {
        let normalized = match k.as_str() {
          // bm25 is segment-layout independent, but f32::ln differs by one ulp between libm
          // implementations; sin/cos/asin behind haversine and centroid likewise.
          "score" => round(v, 3),
          "latitude" | "longitude" => round(v, 4),
          // utoipa fills it from CARGO_PKG_VERSION, so it churns on every release.
          "version" => Value::String("<version>".to_string()),
          _ => normalize(v),
        };
        out.insert(k.clone(), normalized);
      }
      Value::Object(out)
    }
    Value::Array(items) => Value::Array(items.iter().map(normalize).collect()),
    _ => value.clone(),
  }
}

impl world {
  pub fn assert_exact(&self, actual: &Value, expected: &Value) {
    let actual = normalize(actual);
    let expected = normalize(expected);
    if actual != expected {
      panic!(
        "payload differs from the expected literal\n\nexpected:\n{expected:#}\n\nactual:\n{actual:#}"
      );
    }
  }
}
