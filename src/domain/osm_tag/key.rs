#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum osm_tag {
  name,
  place,
  highway,
  leisure,
  building,
  waterway,
  postal_code,
  addr_postcode,
  iso3166_1,
  iso3166_1_alpha2,
}

impl osm_tag {
  pub fn key(self) -> &'static str {
    match self {
      osm_tag::name => "name",
      osm_tag::place => "place",
      osm_tag::highway => "highway",
      osm_tag::leisure => "leisure",
      osm_tag::building => "building",
      osm_tag::waterway => "waterway",
      osm_tag::postal_code => "postal_code",
      osm_tag::addr_postcode => "addr:postcode",
      osm_tag::iso3166_1 => "ISO3166-1",
      osm_tag::iso3166_1_alpha2 => "ISO3166-1:alpha2",
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
