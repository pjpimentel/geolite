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
}

#[cfg(test)]
#[path = "strategy.test.rs"]
mod tests;
