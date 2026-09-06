use super::entity::osm_node;
use crate::domain::osm_pbf_file::message::{dense_nodes_msg, node_msg};
use crate::domain::osm_tag::tag_policy;

#[derive(Clone, Copy)]
pub struct block_scale {
  pub granularity: i64,
  pub lat_offset: i64,
  pub lon_offset: i64,
}

impl block_scale {
  fn to_degrees(self, lat: i64, lon: i64) -> (f64, f64) {
    (
      (self.lat_offset + self.granularity * lat) as f64 * 1e-9,
      (self.lon_offset + self.granularity * lon) as f64 * 1e-9,
    )
  }
}

pub fn decode(
  nodes: &[node_msg],
  strings: &[&str],
  scale: block_scale,
  tags: &tag_policy,
) -> Vec<osm_node> {
  nodes
    .iter()
    .map(|n| {
      let kept = tags.filter(strings, &n.keys, &n.vals);
      let (lat, lon) = scale.to_degrees(n.lat, n.lon);
      osm_node {
        id: n.id,
        lat,
        lon,
        tags: kept
          .into_iter()
          .map(|(k, v)| (k.to_string(), v.to_string()))
          .collect(),
      }
    })
    .collect()
}

pub fn decode_dense(
  dense: &dense_nodes_msg,
  strings: &[&str],
  scale: block_scale,
  tags: &tag_policy,
) -> Vec<osm_node> {
  let mut elements = Vec::new();
  let mut id_acc: i64 = 0;
  let mut lat_acc: i64 = 0;
  let mut lon_acc: i64 = 0;
  let mut kv_pos: usize = 0;

  for i in 0..dense.id.len() {
    id_acc += dense.id[i];
    lat_acc += dense.lat.get(i).copied().unwrap_or(0);
    lon_acc += dense.lon.get(i).copied().unwrap_or(0);

    let (lat, lon) = scale.to_degrees(lat_acc, lon_acc);

    let mut kept = std::collections::HashMap::new();
    loop {
      let k_raw = dense.keys_vals.get(kv_pos).copied().unwrap_or(0);
      kv_pos += 1;
      if k_raw == 0 {
        break;
      }
      let v_raw = dense.keys_vals.get(kv_pos).copied().unwrap_or(0);
      kv_pos += 1;
      let k = strings.get(k_raw as usize).unwrap_or(&"");
      let v = strings.get(v_raw as usize).unwrap_or(&"");
      if tags.passes(k) {
        kept.insert(k.to_string(), v.to_string());
      }
    }

    elements.push(osm_node {
      id: id_acc,
      lat,
      lon,
      tags: kept,
    });
  }

  elements
}

#[cfg(test)]
#[path = "decoder.test.rs"]
mod tests;
