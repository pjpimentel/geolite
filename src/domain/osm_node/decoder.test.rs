use super::*;
use crate::domain::osm_pbf_file::message::dense_nodes_msg;
use crate::domain::osm_pbf_file::osm_data::tag_policy;

fn default_opts() -> tag_policy {
  tag_policy::default()
}

fn scale(granularity: i64, lat_offset: i64, lon_offset: i64) -> block_scale {
  block_scale {
    granularity,
    lat_offset,
    lon_offset,
  }
}

fn make_dense(
  ids: Vec<i64>,
  lats: Vec<i64>,
  lons: Vec<i64>,
  keys_vals: Vec<i32>,
) -> dense_nodes_msg {
  dense_nodes_msg {
    id: ids,
    denseinfo: None,
    lat: lats,
    lon: lons,
    keys_vals,
  }
}

fn has_tag(n: &osm_node, key: &str) -> Option<String> {
  n.tags.get(key).cloned()
}

#[test]
fn _00_accumulates_delta_encoded_node_ids() {
  let dense = make_dense(vec![1, 1, 1], vec![0, 0, 0], vec![0, 0, 0], vec![0, 0, 0]);
  let result = decode_dense(&dense, &[""], scale(100, 0, 0), &default_opts());
  assert_eq!(result.len(), 3);
  assert_eq!(result[0].id, 1);
  assert_eq!(result[1].id, 2);
  assert_eq!(result[2].id, 3);
}

#[test]
fn _01_decodes_lat_lon_with_granularity() {
  let dense = make_dense(vec![1], vec![450_000_000], vec![90_000_000], vec![0]);
  let result = decode_dense(&dense, &[""], scale(100, 0, 0), &default_opts());
  assert!((result[0].lat - 45.0).abs() < 1e-9);
  assert!((result[0].lon - 9.0).abs() < 1e-9);
}

#[test]
fn _02_applies_lat_lon_offsets() {
  let dense = make_dense(vec![1], vec![0], vec![0], vec![0]);
  let result = decode_dense(
    &dense,
    &[""],
    scale(100, 1_000_000_000, 2_000_000_000),
    &default_opts(),
  );
  assert!((result[0].lat - 1.0).abs() < 1e-9);
  assert!((result[0].lon - 2.0).abs() < 1e-9);
}

#[test]
fn _03_resolves_node_tags_from_string_table() {
  let dense = make_dense(vec![1], vec![0], vec![0], vec![1, 2, 3, 4, 0]);
  let strings = ["", "name", "Test", "amenity", "cafe"];
  let result = decode_dense(&dense, &strings, scale(100, 0, 0), &default_opts());
  assert_eq!(has_tag(&result[0], "name"), Some("Test".into()));
  assert_eq!(has_tag(&result[0], "amenity"), Some("cafe".into()));
}

#[test]
fn _04_keeps_nodes_with_and_without_tags() {
  let dense = make_dense(vec![1, 1], vec![0, 0], vec![0, 0], vec![1, 2, 0, 0]);
  let strings = ["", "name", "Test"];
  let result = decode_dense(&dense, &strings, scale(100, 0, 0), &default_opts());
  assert_eq!(result.len(), 2);
  assert_eq!(has_tag(&result[0], "name"), Some("Test".into()));
  assert!(result[1].tags.is_empty());
}

#[test]
fn _05_tags_include_keeps_only_listed_tags() {
  let dense = make_dense(vec![1], vec![0], vec![0], vec![1, 2, 3, 4, 0]);
  let strings = ["", "name", "Test", "amenity", "cafe"];
  let opts = tag_policy {
    include: Some(vec!["name".to_string()]),
    ignore: None,
  };
  let result = decode_dense(&dense, &strings, scale(100, 0, 0), &opts);
  assert_eq!(result[0].tags.len(), 1);
  assert_eq!(has_tag(&result[0], "name"), Some("Test".into()));
}

#[test]
fn _06_tags_ignore_drops_listed_tags() {
  let dense = make_dense(vec![1], vec![0], vec![0], vec![1, 2, 3, 4, 0]);
  let strings = ["", "name", "Test", "amenity", "cafe"];
  let opts = tag_policy {
    include: None,
    ignore: Some(vec!["amenity".to_string()]),
  };
  let result = decode_dense(&dense, &strings, scale(100, 0, 0), &opts);
  assert_eq!(result[0].tags.len(), 1);
  assert_eq!(has_tag(&result[0], "name"), Some("Test".into()));
}
