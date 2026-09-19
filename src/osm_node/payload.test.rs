use super::encode;
use crate::database::jsonb::encoder;
use crate::database::jsonb_fixtures::{tags, to_json};
use crate::osm_node::osm_node;

fn encoded(node: &osm_node) -> serde_json::Value {
  let mut out = Vec::new();
  encode(&mut encoder::new(), &mut out, node);
  to_json(&out)
}

#[test]
fn _00_encodes_the_node_as_lat_lon_and_a_tags_object() {
  let json = encoded(&osm_node {
    id: 7,
    lat: 38.7,
    lon: -9.1,
    tags: tags(&[("name", "Marco Zero")]),
  });

  assert!((json["lat"].as_f64().expect("lat") - 38.7).abs() < 1e-9);
  assert!((json["lon"].as_f64().expect("lon") - -9.1).abs() < 1e-9);
  assert_eq!(json["tags"]["name"], "Marco Zero");
}

#[test]
fn _01_encodes_a_node_without_tags_as_an_empty_object() {
  let json = encoded(&osm_node {
    id: 8,
    lat: 0.0,
    lon: 0.0,
    tags: std::collections::HashMap::new(),
  });

  assert_eq!(json["tags"], serde_json::json!({}));
}
