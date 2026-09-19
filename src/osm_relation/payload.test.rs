use super::encode;
use crate::database::jsonb::encoder;
use crate::database::jsonb_fixtures::{tags, to_json};
use crate::osm_relation::entity::{osm_member_type, osm_relation, osm_relation_member};

fn member(osm_member_type: osm_member_type, id: i64, role: &str) -> osm_relation_member {
  osm_relation_member {
    osm_member_type,
    id,
    role: role.to_string(),
  }
}

#[test]
fn _00_encodes_every_member_type_with_its_code() {
  let mut out = Vec::new();
  encode(
    &mut encoder::new(),
    &mut out,
    &osm_relation {
      id: 200,
      tags: tags(&[("name", "Lisboa")]),
      members: vec![
        member(osm_member_type::node, 1, "admin_centre"),
        member(osm_member_type::way, 2, "outer"),
        member(osm_member_type::relation, 3, "subarea"),
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
