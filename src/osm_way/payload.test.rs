use super::encode;
use crate::database::jsonb::encoder;
use crate::database::jsonb_fixtures::{tags, to_json};
use crate::osm_way::osm_way;

#[test]
fn _00_encodes_the_refs_as_an_array_and_the_tags_as_an_object() {
  let mut out = Vec::new();
  encode(
    &mut encoder::new(),
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
