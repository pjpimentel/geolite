use geo::Point;

use super::value::{house_number, house_number_shape};

// where a door number landed on the street the user asked about.
#[derive(Debug, PartialEq)]
pub enum house_number_resolution {
  // a number with this exact value is mapped on the street.
  exact(Point<f64>),
  // the number is not mapped, but it falls between two that are, so its position is the linear
  // interpolation between them.
  interpolated(Point<f64>),
  // a number was asked for and could not be placed. the street is still a valid answer.
  absent,
}

// `known` holds the numbers mapped on one street, each with the point it sits at. order does not
// matter; the first exact match wins, as it did when this ran off a database cursor.
pub fn resolve(
  wanted: &house_number,
  known: &[(house_number, Point<f64>)],
) -> house_number_resolution {
  if let Some((_, point)) = known.iter().find(|(number, _)| number == wanted) {
    return house_number_resolution::exact(*point);
  }
  // interpolating a compound number would be meaningless: its leading part names a cross street,
  // not a position along this one, so numbers do not run in order.
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

// bracketing: the nearest known number below and above the target, then a straight lerp between
// their points. without both sides there is nothing to interpolate between.
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
