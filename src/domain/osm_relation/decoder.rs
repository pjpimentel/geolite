use super::entity::{osm_member_type, osm_relation, osm_relation_member};
use crate::pbf::message::relation_msg;
use crate::pbf::tag_policy::tag_policy;

// member ids are delta-encoded against the previous member, and the role is an index into the
// block's string table. the three parallel arrays — memids, roles_sid, types — line up by
// position, so a short or missing entry falls back rather than shifting everything after it.
pub fn decode(
  relations: &[relation_msg],
  strings: &[&str],
  tags: &tag_policy,
) -> Vec<osm_relation> {
  let mut elements = Vec::new();

  for r in relations {
    let kept = tags.filter(strings, &r.keys, &r.vals);
    let mut memid_acc: i64 = 0;
    let members: Vec<osm_relation_member> = r
      .memids
      .iter()
      .enumerate()
      .map(|(i, &d)| {
        memid_acc += d;
        let role_idx = r.roles_sid.get(i).copied().unwrap_or(0) as usize;
        let role = strings.get(role_idx).unwrap_or(&"").to_string();
        let osm_member_type = match r.types.get(i).copied().unwrap_or(0) {
          1 => osm_member_type::way,
          2 => osm_member_type::relation,
          _ => osm_member_type::node,
        };
        osm_relation_member {
          osm_member_type,
          id: memid_acc,
          role,
        }
      })
      .collect();
    elements.push(osm_relation {
      id: r.id,
      tags: kept
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
      members,
    });
  }

  elements
}

#[cfg(test)]
#[path = "decoder.test.rs"]
mod tests;
