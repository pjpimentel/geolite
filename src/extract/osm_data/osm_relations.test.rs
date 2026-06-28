fn default_opts() -> super::super::data_opts {
  super::super::data_opts {
    include_nodes: true,
    include_ways: true,
    include_relations: true,
    ignore_info: true,
    tags_include: None,
    tags_ignore: None,
    buffer_bytes: 1_073_741_824,
  }
}

fn make_relation(
  id: i64,
  keys: Vec<u32>,
  vals: Vec<u32>,
  roles_sid: Vec<i32>,
  memids: Vec<i64>,
  types: Vec<i32>,
) -> super::super::relation_msg {
  super::super::relation_msg {
    id,
    keys,
    vals,
    info: None,
    roles_sid,
    memids,
    types,
  }
}

// 0: delta-decode de memids multi-member → [5, 3, -2] acumula em [5, 8, 6]
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
  let result = super::decode(&[rel], &[""], &default_opts());
  assert_eq!(result.len(), 1);
  assert_eq!(result[0].members.len(), 3);
  assert_eq!(result[0].members[0].id, 5);
  assert_eq!(result[0].members[1].id, 8);
  assert_eq!(result[0].members[2].id, 6);
}

// 1: types=[0, 1, 2] → [node, way, relation], cobrindo todos os bracos do match
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
  let result = super::decode(&[rel], &[""], &default_opts());
  assert!(matches!(
    result[0].members[0].osm_member_type,
    super::osm_member_type::node
  ));
  assert!(matches!(
    result[0].members[1].osm_member_type,
    super::osm_member_type::way
  ));
  assert!(matches!(
    result[0].members[2].osm_member_type,
    super::osm_member_type::relation
  ));
}

// 2: roles_sid=[1, 2] resolvidos via string table → ["outer", "inner"]
#[test]
fn _02_resolves_member_roles_from_string_table() {
  let rel = make_relation(1, vec![], vec![], vec![1, 2], vec![10, 0], vec![1, 1]);
  let strings = ["", "outer", "inner"];
  let result = super::decode(&[rel], &strings, &default_opts());
  assert_eq!(result[0].members[0].role, "outer");
  assert_eq!(result[0].members[1].role, "inner");
}

// 3: roles_sid mais curto que memids → member faltante cai em strings[0]=""
#[test]
fn _03_missing_role_falls_back_to_empty_string() {
  let rel = make_relation(1, vec![], vec![], vec![1], vec![1, 2], vec![1, 1]);
  let strings = ["", "outer"];
  let result = super::decode(&[rel], &strings, &default_opts());
  assert_eq!(result[0].members.len(), 2);
  assert_eq!(result[0].members[0].role, "outer");
  assert_eq!(result[0].members[1].role, "");
}

// 4: multiplas relations preservam ordem e r.id NAO e delta → [100, 200] fica [100, 200]
#[test]
fn _04_preserves_order_of_multiple_relations() {
  let r1 = make_relation(100, vec![], vec![], vec![], vec![], vec![]);
  let r2 = make_relation(200, vec![], vec![], vec![], vec![], vec![]);
  let result = super::decode(&[r1, r2], &[""], &default_opts());
  assert_eq!(result.len(), 2);
  assert_eq!(result[0].id, 100);
  assert_eq!(result[1].id, 200);
}
