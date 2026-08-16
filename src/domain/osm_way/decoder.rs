use super::entity::osm_way;
use crate::pbf::message::way_msg;
use crate::pbf::tag_policy::tag_policy;

// node references are delta-encoded against the previous one, so the list has to be walked in
// order and accumulated — the same trick the dense node form uses for ids and coordinates.
pub fn decode(ways: &[way_msg], strings: &[&str], tags: &tag_policy) -> Vec<osm_way> {
  let mut elements = Vec::new();

  for w in ways {
    let kept = tags.filter(strings, &w.keys, &w.vals);
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
      tags: kept
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
    });
  }

  elements
}

#[cfg(test)]
#[path = "decoder.test.rs"]
mod tests;
