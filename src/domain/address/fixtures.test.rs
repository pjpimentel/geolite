use crate::domain::address::{admin_level, query_match, query_match_attributes};

pub(crate) fn admin_level_at(level: u8, name: &str) -> admin_level {
  admin_level {
    level,
    name: name.to_string(),
    osm_relation_id: None,
    osm_way_id: None,
    wkt: None,
  }
}

pub(crate) fn match_at(id: u64, lat: f64, lon: f64, admin_levels: Vec<admin_level>) -> query_match {
  query_match {
    admin_levels,
    latitude: lat,
    longitude: lon,
    coordinates_distance_in_meters: None,
    similarity: None,
    score: None,
    friendly_name: String::new(),
    attributes: query_match_attributes {
      country_iso_3166_1_alpha_2_code: None,
      post_code: None,
    },
    house_number: None,
    id,
    admin_level_id: None,
  }
}
