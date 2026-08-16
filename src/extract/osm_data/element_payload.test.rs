use super::*;

use crate::database::jsonb::encoder;
use crate::extract::osm_data::osm_relations::{
  osm_member_type, osm_relation, osm_relation_member,
};
use crate::extract::osm_data::osm_ways::osm_way;

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
  encode_way(

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

// 00.03: relation carrega os tres tipos de membro, cada um com sua sigla
#[test]
fn _00_03_encodes_relation_with_every_member_type() {
  let mut enc = encoder::new();
  let mut out = Vec::new();
  encode_relation(

    &mut enc,
    &mut out,
    &osm_relation {
      id: 200,
      tags: tags(&[("name", "Lisboa")]),
      members: vec![
        osm_relation_member {
          osm_member_type: osm_member_type::node,
          id: 1,
          role: "admin_centre".to_string(),
        },
        osm_relation_member {
          osm_member_type: osm_member_type::way,
          id: 2,
          role: "outer".to_string(),
        },
        osm_relation_member {
          osm_member_type: osm_member_type::relation,
          id: 3,
          role: "subarea".to_string(),
        },
      ],
    },
  );

  let json = to_json(&out);
  assert_eq!(json["tags"]["name"], "Lisboa");
  assert_eq!(
    json["members"],
    serde_json::json!([
      { "type": "n", "id": 1, "role": "admin_centre" },
      { "type": "w", "id": 2, "role": "outer" },
      { "type": "r", "id": 3, "role": "subarea" },
    ])
  );
}
