#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub enum osm_tag {
  addr_housenumber,
  addr_postcode,
  addr_street,
  admin_level,
  building,
  highway,
  iso3166_1,
  iso3166_1_alpha2,
  leisure,
  name,
  place,
  postal_code,
  waterway,
}

impl osm_tag {
  pub fn key(self) -> &'static str {
    match self {
      osm_tag::addr_housenumber => "addr:housenumber",
      osm_tag::addr_postcode => "addr:postcode",
      osm_tag::addr_street => "addr:street",
      osm_tag::admin_level => "admin_level",
      osm_tag::building => "building",
      osm_tag::highway => "highway",
      osm_tag::iso3166_1 => "ISO3166-1",
      osm_tag::iso3166_1_alpha2 => "ISO3166-1:alpha2",
      osm_tag::leisure => "leisure",
      osm_tag::name => "name",
      osm_tag::place => "place",
      osm_tag::postal_code => "postal_code",
      osm_tag::waterway => "waterway",
    }
  }

  pub fn json_path(self) -> String {
    json_path_of(self.key())
  }
}

pub const POST_CODE: &[osm_tag] = &[osm_tag::postal_code, osm_tag::addr_postcode];
pub const COUNTRY_ISO: &[osm_tag] = &[osm_tag::iso3166_1, osm_tag::iso3166_1_alpha2];

pub fn json_path_of(key: &str) -> String {
  format!("$.tags.\"{key}\"")
}

pub fn is_valid_key(key: &str) -> bool {
  !key.is_empty()
    && key
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '-' || c == '_')
}

#[cfg(test)]
#[path = "key.test.rs"]
mod tests;
