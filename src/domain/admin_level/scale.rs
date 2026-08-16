// the administrative level of a named area, from continent down to the door number.
//
// osm tags levels 1..11 on boundary relations. geolite extends the scale past that: 12 for streets,
// which are mapped as ways rather than boundaries, and 30 for house numbers, which are synthesized
// while answering a query and never stored as an area.
//
// the set is closed on purpose. a level this type cannot name is a level the pipeline cannot
// extract, so it is rejected at the edge that reads it instead of travelling as a bare integer and
// silently matching nothing. that is also why `name` needs no fallback: an unnamed level is not
// representable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum level {
  continent = 1,
  country = 2,
  region = 3,
  state = 4,
  district = 5,
  county = 6,
  municipality = 7,
  city = 8,
  locality = 9,
  neighborhood = 10,
  street = 12,
  address = 14,
  house_number = 30,
}

impl level {
  // the only fallible entry point: parsing a level that came from outside — a cli flag, a query
  // string, a database column.
  pub fn new(value: u8) -> Option<Self> {
    match value {
      1 => Some(level::continent),
      2 => Some(level::country),
      3 => Some(level::region),
      4 => Some(level::state),
      5 => Some(level::district),
      6 => Some(level::county),
      7 => Some(level::municipality),
      8 => Some(level::city),
      9 => Some(level::locality),
      10 => Some(level::neighborhood),
      12 => Some(level::street),
      14 => Some(level::address),
      30 => Some(level::house_number),
      _ => None,
    }
  }

  pub fn value(self) -> u8 {
    self as u8
  }

  pub fn name(self) -> &'static str {
    match self {
      level::continent => "continent",
      level::country => "country",
      level::region => "region",
      level::state => "state",
      level::district => "district",
      level::county => "county",
      level::municipality => "municipality",
      level::city => "city",
      level::locality => "locality",
      level::neighborhood => "neighborhood",
      level::street => "street",
      level::address => "address",
      level::house_number => "house_number",
    }
  }

  pub fn is_street(self) -> bool {
    matches!(self, level::street)
  }

  #[allow(dead_code)]
  pub fn is_house_number(self) -> bool {
    matches!(self, level::house_number)
  }
}

// ordering is by the level value: a higher level is more specific, so a street sorts after the city
// that contains it. the derived ordering would follow declaration order instead, which would change
// meaning the day a variant is moved — hence the explicit implementation.
impl Ord for level {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.value().cmp(&other.value())
  }
}

impl PartialOrd for level {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

#[cfg(test)]
#[path = "scale.test.rs"]
mod tests;
