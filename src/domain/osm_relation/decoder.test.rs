use super::*;
use crate::domain::osm_relation::entity::osm_member_type;
use relation_msg;
use crate::pbf::tag_policy::tag_policy;

fn default_opts() -> tag_policy {
  tag_policy::default()
}

fn make_relation(
  id: i64,
  keys: Vec<u32>,
  vals: Vec<u32>,
  roles_sid: Vec<i32>,
  memids: Vec<i64>,
  types: Vec<i32>,
) -> relation_msg {
  relation_msg {
    id,
    keys,
    vals,
    info: None,
    roles_sid,
    memids,
    types,
  }
}

#[test]
fn _00_delta_decodes_member_ids() {
  let rel = make_relation(
    1,
    vec![],
    vec![],
    vec![0, 0, 0],
    vec![5, 3, -2],
    vec![0, 0, 0],
  );
  let result = decode(&[rel], &[""], &default_opts());
  assert_eq!(result.len(), 1);
  assert_eq!(result[0].members.len(), 3);
  assert_eq!(result[0].members[0].id, 5);
  assert_eq!(result[0].members[1].id, 8);
  assert_eq!(result[0].members[2].id, 6);
}

#[test]
fn _01_maps_member_types_node_way_relation() {
  let rel = make_relation(
    1,
    vec![],
    vec![],
    vec![0, 0, 0],
    vec![1, 1, 1],
    vec![0, 1, 2],
  );
  let result = decode(&[rel], &[""], &default_opts());
  assert!(matches!(
    result[0].members[0].osm_member_type,
    osm_member_type::node
  ));
  assert!(matches!(
    result[0].members[1].osm_member_type,
    osm_member_type::way
  ));
  assert!(matches!(
    result[0].members[2].osm_member_type,
    osm_member_type::relation
  ));
}

#[test]
fn _02_resolves_member_roles_from_string_table() {
  let rel = make_relation(1, vec![], vec![], vec![1, 2], vec![10, 0], vec![1, 1]);
  let strings = ["", "outer", "inner"];
  let result = decode(&[rel], &strings, &default_opts());
  assert_eq!(result[0].members[0].role, "outer");
  assert_eq!(result[0].members[1].role, "inner");
}

#[test]
fn _03_missing_role_falls_back_to_empty_string() {
  let rel = make_relation(1, vec![], vec![], vec![1], vec![1, 2], vec![1, 1]);
  let strings = ["", "outer"];
  let result = decode(&[rel], &strings, &default_opts());
  assert_eq!(result[0].members.len(), 2);
  assert_eq!(result[0].members[0].role, "outer");
  assert_eq!(result[0].members[1].role, "");
}

#[test]
fn _04_preserves_order_of_multiple_relations() {
  let r1 = make_relation(100, vec![], vec![], vec![], vec![], vec![]);
  let r2 = make_relation(200, vec![], vec![], vec![], vec![], vec![]);
  let result = decode(&[r1, r2], &[""], &default_opts());
  assert_eq!(result.len(), 2);
  assert_eq!(result[0].id, 100);
  assert_eq!(result[1].id, 200);
}
