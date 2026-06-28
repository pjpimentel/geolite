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

fn make_dense(
  ids: Vec<i64>,
  lats: Vec<i64>,
  lons: Vec<i64>,
  keys_vals: Vec<i32>,
) -> super::super::dense_nodes_msg {
  super::super::dense_nodes_msg {
    id: ids,
    denseinfo: None,
    lat: lats,
    lon: lons,
    keys_vals,
  }
}

fn has_tag(n: &super::osm_node, key: &str) -> Option<String> {
  n.tags.get(key).cloned()
}

// 0: múltiplos nodes com delta ids [1,1,1] → ids acumulados [1,2,3]
#[test]
fn _00_accumulates_delta_encoded_node_ids() {
  let dense = make_dense(vec![1, 1, 1], vec![0, 0, 0], vec![0, 0, 0], vec![0, 0, 0]);
  let result = super::decode_dense_nodes(&dense, &[""], 100, 0, 0, 1000, &default_opts());
  assert_eq!(result.len(), 3);
  assert_eq!(result[0].id, 1);
  assert_eq!(result[1].id, 2);
  assert_eq!(result[2].id, 3);
}

// 1: lat/lon com granularity=100 e offsets=0
// lat = (0 + 100 * 450_000_000) * 1e-9 = 45.0
// lon = (0 + 100 *  90_000_000) * 1e-9 =  9.0
#[test]
fn _01_decodes_lat_lon_with_granularity() {
  let dense = make_dense(vec![1], vec![450_000_000], vec![90_000_000], vec![0]);
  let result = super::decode_dense_nodes(&dense, &[""], 100, 0, 0, 1000, &default_opts());
  assert!((result[0].lat - 45.0).abs() < 1e-9);
  assert!((result[0].lon - 9.0).abs() < 1e-9);
}

// 2: lat/lon com lat_offset e lon_offset não-zero
// lat = (1_000_000_000 + 100 * 0) * 1e-9 = 1.0
// lon = (2_000_000_000 + 100 * 0) * 1e-9 = 2.0
#[test]
fn _02_applies_lat_lon_offsets() {
  let dense = make_dense(vec![1], vec![0], vec![0], vec![0]);
  let result = super::decode_dense_nodes(
    &dense,
    &[""],
    100,
    1_000_000_000,
    2_000_000_000,
    1000,
    &default_opts(),
  );
  assert!((result[0].lat - 1.0).abs() < 1e-9);
  assert!((result[0].lon - 2.0).abs() < 1e-9);
}

// 3 único node com 2 tags via keys_vals=[1,2,3,4,0]
// strings=["","name","Test","amenity","cafe"] → tags=[("name","Test"),("amenity","cafe")]
#[test]
fn _03_resolves_node_tags_from_string_table() {
  let dense = make_dense(vec![1], vec![0], vec![0], vec![1, 2, 3, 4, 0]);
  let strings = ["", "name", "Test", "amenity", "cafe"];
  let result = super::decode_dense_nodes(&dense, &strings, 100, 0, 0, 1000, &default_opts());
  assert_eq!(has_tag(&result[0], "name"), Some("Test".into()));
  assert_eq!(has_tag(&result[0], "amenity"), Some("cafe".into()));
}

// 4: 2 nodes: primeiro com tag, segundo sem tags
// keys_vals=[1,2,0,0] → node0:("name","Test"), node1: sem tags
#[test]
fn _04_keeps_nodes_with_and_without_tags() {
  let dense = make_dense(vec![1, 1], vec![0, 0], vec![0, 0], vec![1, 2, 0, 0]);
  let strings = ["", "name", "Test"];
  let result = super::decode_dense_nodes(&dense, &strings, 100, 0, 0, 1000, &default_opts());
  assert_eq!(result.len(), 2);
  assert_eq!(has_tag(&result[0], "name"), Some("Test".into()));
  assert!(result[1].tags.is_empty());
}

// 5: tags_include=["name"] num node com tags {name:"Test",amenity:"cafe"}
// → apenas ("name","Test")
#[test]
fn _05_tags_include_keeps_only_listed_tags() {
  let dense = make_dense(vec![1], vec![0], vec![0], vec![1, 2, 3, 4, 0]);
  let strings = ["", "name", "Test", "amenity", "cafe"];
  let opts = super::super::data_opts {
    tags_include: Some(vec!["name".to_string()]),
    tags_ignore: None,
    ..default_opts()
  };
  let result = super::decode_dense_nodes(&dense, &strings, 100, 0, 0, 1000, &opts);
  assert_eq!(result[0].tags.len(), 1);
  assert_eq!(has_tag(&result[0], "name"), Some("Test".into()));
}

// 6: tags_ignore=["amenity"] num node com tags {name:"Test",amenity:"cafe"}
// → apenas ("name","Test")
#[test]
fn _06_tags_ignore_drops_listed_tags() {
  let dense = make_dense(vec![1], vec![0], vec![0], vec![1, 2, 3, 4, 0]);
  let strings = ["", "name", "Test", "amenity", "cafe"];
  let opts = super::super::data_opts {
    tags_include: None,
    tags_ignore: Some(vec!["amenity".to_string()]),
    ..default_opts()
  };
  let result = super::decode_dense_nodes(&dense, &strings, 100, 0, 0, 1000, &opts);
  assert_eq!(result[0].tags.len(), 1);
  assert_eq!(has_tag(&result[0], "name"), Some("Test".into()));
}
