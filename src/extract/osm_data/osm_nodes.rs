#[derive(serde::Serialize, serde::Deserialize)]
pub struct osm_node {
  #[serde(skip_serializing, default)]
  pub id: i64,
  pub lat: f64,
  pub lon: f64,
  pub tags: std::collections::HashMap<String, String>,
}

pub(super) fn decode_nodes(
  nodes: &[super::node_msg],
  strings: &[&str],
  granularity: i64,
  lat_offset: i64,
  lon_offset: i64,
  _date_granularity: i64,
  opts: &super::data_opts,
) -> Vec<osm_node> {
  nodes
    .iter()
    .map(|n| {
      let tags = super::filter_tags(strings, &n.keys, &n.vals, opts);
      let lat = (lat_offset + granularity * n.lat) as f64 * 1e-9;
      let lon = (lon_offset + granularity * n.lon) as f64 * 1e-9;
      osm_node {
        id: n.id,
        lat,
        lon,
        tags: tags
          .into_iter()
          .map(|(k, v)| (k.to_string(), v.to_string()))
          .collect(),
      }
    })
    .collect()
}

pub(super) fn decode_dense_nodes(
  dense: &super::dense_nodes_msg,
  strings: &[&str],
  granularity: i64,
  lat_offset: i64,
  lon_offset: i64,
  _date_granularity: i64,
  opts: &super::data_opts,
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

    let lat = (lat_offset + granularity * lat_acc) as f64 * 1e-9;
    let lon = (lon_offset + granularity * lon_acc) as f64 * 1e-9;

    let mut tags = std::collections::HashMap::new();
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
      if super::tag_passes(k, opts) {
        tags.insert(k.to_string(), v.to_string());
      }
    }

    elements.push(osm_node {
      id: id_acc,
      lat,
      lon,
      tags,
    });
  }

  elements
}

#[cfg(test)]
#[path = "osm_nodes.test.rs"]
mod decode_osm_nodes_test;
