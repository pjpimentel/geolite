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

fn make_way(id: i64, keys: Vec<u32>, vals: Vec<u32>, refs: Vec<i64>) -> super::super::way_msg {
  super::super::way_msg {
    id,
    keys,
    vals,
    info: None,
    refs,
    lat: vec![],
    lon: vec![],
  }
}

// 0: delta-decode de refs multi-ref → [10, 10, -5] acumula em [10, 20, 15]
#[test]
fn _00_delta_decodes_way_refs() {
  let w = make_way(1, vec![], vec![], vec![10, 10, -5]);
  let result = super::decode(&[w], &[""], &default_opts());
  assert_eq!(result.len(), 1);
  assert_eq!(result[0].refs, vec![10, 20, 15]);
}

// 1: multiplas ways preservam ordem e w.id NAO e delta → [100, 200] fica [100, 200]
#[test]
fn _01_preserves_order_of_multiple_ways() {
  let w1 = make_way(100, vec![], vec![], vec![]);
  let w2 = make_way(200, vec![], vec![], vec![]);
  let result = super::decode(&[w1, w2], &[""], &default_opts());
  assert_eq!(result.len(), 2);
  assert_eq!(result[0].id, 100);
  assert_eq!(result[1].id, 200);
}
