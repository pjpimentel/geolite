#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum link_strategy {
  by_proximity,
  by_name,
}

impl link_strategy {
  pub fn code(self) -> u8 {
    match self {
      link_strategy::by_proximity => 0,
      link_strategy::by_name => 1,
    }
  }

  #[allow(dead_code)]
  pub fn from_code(code: u8) -> Option<Self> {
    match code {
      0 => Some(link_strategy::by_proximity),
      1 => Some(link_strategy::by_name),
      _ => None,
    }
  }
}

#[cfg(test)]
#[path = "strategy.test.rs"]
mod tests;
