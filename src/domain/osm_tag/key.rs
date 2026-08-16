// the osm tag keys geolite interprets, and how each one is written.
//
// the set is **not** the set of tags geolite stores — `pbf::tag_policy` keeps whatever the file
// carries, including keys nobody here has heard of. this enum names only the keys the code reasons
// about, so that a misspelling is a compile error rather than a query that quietly returns nothing.

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

// the same key written twice by osm; the first present wins.
pub const POST_CODE: &[osm_tag] = &[osm_tag::postal_code, osm_tag::addr_postcode];
pub const COUNTRY_ISO: &[osm_tag] = &[osm_tag::iso3166_1, osm_tag::iso3166_1_alpha2];

// the json path of a key that arrives as free text — `--name-priority`, a preset's `number_tags`.
//
// the key is **always** quoted. sqlite tolerates a bare key for most characters (`:` and `-`
// included, which is why the hand-written paths this replaced worked), but a `.` reads as a nested
// path and a `[` as an array index — both resolve to NULL with no error at all. quoting always is
// the only form that cannot be silently wrong, and it is what the name select has always done.
pub fn json_path_of(key: &str) -> String {
  format!("$.tags.\"{key}\"")
}

// what may be used as a tag key. it excludes `.` and `[`, the two characters a bare json path would
// misread, so a key that passes here is safe in either form.
pub fn is_valid_key(key: &str) -> bool {
  !key.is_empty()
    && key
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '-' || c == '_')
}

#[cfg(test)]
#[path = "key.test.rs"]
mod tests;
