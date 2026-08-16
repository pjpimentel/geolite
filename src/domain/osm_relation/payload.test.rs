use super::*;

use crate::database::jsonb::encoder;
use crate::domain::osm_relation::entity::{osm_member_type, osm_relation, osm_relation_member};

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

#[test]
fn _00_encodes_relation_with_every_member_type() {
  let mut enc = encoder::new();
  let mut out = Vec::new();
  encode(
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
