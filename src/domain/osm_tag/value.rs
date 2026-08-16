#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum place_value {
  neighbourhood,
  suburb,
}

impl place_value {
  pub fn value(self) -> &'static str {
    match self {
      place_value::neighbourhood => "neighbourhood",
      place_value::suburb => "suburb",
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub enum highway_value {
  residential,
  primary,
  secondary,
  tertiary,
  unclassified,
  living_street,
}

impl highway_value {
  pub fn value(self) -> &'static str {
    match self {
      highway_value::residential => "residential",
      highway_value::primary => "primary",
      highway_value::secondary => "secondary",
      highway_value::tertiary => "tertiary",
      highway_value::unclassified => "unclassified",
      highway_value::living_street => "living_street",
    }
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum leisure_value {
  park,
}

impl leisure_value {
  pub fn value(self) -> &'static str {
    match self {
      leisure_value::park => "park",
    }
  }
}
