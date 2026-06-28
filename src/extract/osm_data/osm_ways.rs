#[derive(serde::Serialize, serde::Deserialize)]
pub struct osm_way {
  #[serde(skip_serializing, default)]
  pub id: i64,
  pub refs: Vec<i64>,
  pub tags: std::collections::HashMap<String, String>,
}

pub(super) fn decode(
  ways: &[super::way_msg],
  strings: &[&str],
  opts: &super::data_opts,
) -> Vec<osm_way> {
  let mut elements = Vec::new();

  for w in ways {
    let tags = super::filter_tags(strings, &w.keys, &w.vals, opts);
    let mut ref_acc: i64 = 0;
    let refs: Vec<i64> = w
      .refs
      .iter()
      .map(|&d| {
        ref_acc += d;
        ref_acc
      })
      .collect();
    elements.push(osm_way {
      id: w.id,
      refs,
      tags: tags
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
    });
  }

  elements
}

#[cfg(test)]
#[path = "osm_ways.test.rs"]
mod decode_osm_ways_test;
