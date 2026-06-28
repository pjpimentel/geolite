#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct osm_relation {
  #[serde(skip_serializing, default)]
  pub id: i64,
  pub tags: std::collections::HashMap<String, String>,
  pub members: Vec<osm_relation_member>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) enum osm_member_type {
  #[serde(rename = "n")]
  node = 0,
  #[serde(rename = "w")]
  way = 1,
  #[serde(rename = "r")]
  relation = 2,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct osm_relation_member {
  #[serde(rename = "type")]
  pub osm_member_type: osm_member_type,
  pub id: i64,
  pub role: String,
}

pub(super) fn decode(
  relations: &[super::relation_msg],
  strings: &[&str],
  opts: &super::data_opts,
) -> Vec<osm_relation> {
  let mut elements = Vec::new();

  for r in relations {
    let tags = super::filter_tags(strings, &r.keys, &r.vals, opts);
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
      tags: tags
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
      members,
    });
  }

  elements
}

#[cfg(test)]
#[path = "osm_relations.test.rs"]
mod decode_osm_relations_test;
