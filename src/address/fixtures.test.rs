use crate::address::admin_level;

pub(crate) fn admin_level_at(level: u8, name: &str) -> admin_level {
  admin_level {
    level,
    name: name.to_string(),
    post_code: None,
    osm_relation_id: None,
    osm_way_id: None,
    osm_merged_way_ids: None,
    wkt: None,
  }
}
