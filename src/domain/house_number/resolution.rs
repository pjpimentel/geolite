use geo::Point;

use super::value::{house_number, house_number_shape};

#[derive(Debug, PartialEq)]
pub enum house_number_resolution {
  exact(Point<f64>),
  interpolated(Point<f64>),
  absent,
}

pub fn resolve(
  wanted: &house_number,
  known: &[(house_number, Point<f64>)],
) -> house_number_resolution {
  if let Some((_, point)) = known.iter().find(|(number, _)| number == wanted) {
    return house_number_resolution::exact(*point);
  }
  if wanted.shape() == house_number_shape::compound {
    return house_number_resolution::absent;
  }
  match wanted
    .leading_value()
    .and_then(|target| interpolate(known, target))
  {
    Some(point) => house_number_resolution::interpolated(point),
    None => house_number_resolution::absent,
  }
}

fn interpolate(known: &[(house_number, Point<f64>)], target: u32) -> Option<Point<f64>> {
  let mut below: Option<(u32, Point<f64>)> = None;
  let mut above: Option<(u32, Point<f64>)> = None;

  for (number, point) in known {
    let Some(value) = number.leading_value() else {
      continue;
    };
    if value < target {
      if below.is_none_or(|(current, _)| value > current) {
        below = Some((value, *point));
      }
    } else if value > target && above.is_none_or(|(current, _)| value < current) {
      above = Some((value, *point));
    }
  }

  let ((low, low_point), (high, high_point)) = (below?, above?);
  let fraction = (target - low) as f64 / (high - low) as f64;
  Some(Point::new(
    low_point.x() + (high_point.x() - low_point.x()) * fraction,
    low_point.y() + (high_point.y() - low_point.y()) * fraction,
  ))
}

#[cfg(test)]
#[path = "resolution.test.rs"]
mod tests;
