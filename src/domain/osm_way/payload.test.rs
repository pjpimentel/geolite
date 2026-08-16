use super::*;

use crate::database::jsonb::encoder;
use crate::domain::osm_way::osm_way;

// sqlite itself is the oracle for the format: a malformed jsonb makes JSON() fail or return
// something other than what was written.
fn to_json(payload: &[u8]) -> serde_json::Value {
  let conn = rusqlite::Connection::open_in_memory().expect("failed to open sqlite");
  let text: String = conn
    .query_row("SELECT JSON(?1)", rusqlite::params![payload], |r| r.get(0))
    .expect("sqlite nao conseguiu ler o jsonb produzido");
  serde_json::from_str(&text).expect("sqlite devolveu json invalido")
}

fn tags(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
  pairs
    .iter()
    .map(|&(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

// 00.02: way vira objeto com array de refs e objeto de tags
#[test]
fn _00_02_encodes_way_with_refs_array() {
  let mut enc = encoder::new();
  let mut out = Vec::new();
  encode(
    &mut enc,
    &mut out,
    &osm_way {
      id: 100,
      refs: vec![1, 2, -3],
      tags: tags(&[("highway", "residential")]),
    },
  );

  let json = to_json(&out);
  assert_eq!(json["refs"], serde_json::json!([1, 2, -3]));
  assert_eq!(json["tags"]["highway"], "residential");
}
