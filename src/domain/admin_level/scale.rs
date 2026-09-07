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

  pub fn parse(text: &str) -> Result<Self, level_error> {
    let text = text.trim();
    let value: u8 = text
      .parse()
      .map_err(|_| level_error::not_a_level_number(text.to_string()))?;
    level::new(value).ok_or(level_error::outside_the_scale(value))
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
}

#[derive(Debug, PartialEq, Eq)]
pub enum level_error {
  not_a_level_number(String),
  outside_the_scale(u8),
}

impl std::fmt::Display for level_error {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      level_error::not_a_level_number(text) => write!(f, "'{text}' is not a level number"),
      level_error::outside_the_scale(value) => write!(f, "level {value} is not supported"),
    }
  }
}

impl std::error::Error for level_error {}

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
