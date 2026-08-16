use super::*;
use way_msg;
use crate::pbf::tag_policy::tag_policy;

fn default_opts() -> tag_policy {
  tag_policy::default()
}

fn make_way(id: i64, keys: Vec<u32>, vals: Vec<u32>, refs: Vec<i64>) -> way_msg {
  way_msg {
    id,
    keys,
    vals,
    info: None,
    refs,
    lat: vec![],
    lon: vec![],
  }
}

#[test]
fn _00_delta_decodes_way_refs() {
  let w = make_way(1, vec![], vec![], vec![10, 10, -5]);
  let result = decode(&[w], &[""], &default_opts());
  assert_eq!(result.len(), 1);
  assert_eq!(result[0].refs, vec![10, 20, 15]);
}

#[test]
fn _01_preserves_order_of_multiple_ways() {
  let w1 = make_way(100, vec![], vec![], vec![]);
  let w2 = make_way(200, vec![], vec![], vec![]);
  let result = decode(&[w1, w2], &[""], &default_opts());
  assert_eq!(result.len(), 2);
  assert_eq!(result[0].id, 100);
  assert_eq!(result[1].id, 200);
}
