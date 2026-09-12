#[derive(Debug, PartialEq)]
pub enum query_input {
  coordinates { latitude: f64, longitude: f64 },
  text,
}

impl query_input {
  pub fn parse(raw: &str) -> query_input {
    let Some((latitude, longitude)) = raw.trim().split_once(',') else {
      return query_input::text;
    };
    let (Ok(latitude), Ok(longitude)) = (
      latitude.trim().parse::<f64>(),
      longitude.trim().parse::<f64>(),
    ) else {
      return query_input::text;
    };
    if (-90.0..=90.0).contains(&latitude) && (-180.0..=180.0).contains(&longitude) {
      query_input::coordinates {
        latitude,
        longitude,
      }
    } else {
      query_input::text
    }
  }
}

#[cfg(test)]
#[path = "input.test.rs"]
mod tests;
