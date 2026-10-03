use geo::{Geometry, HaversineDistance, Point};

use super::axis::street_axis;
use super::policy::house_number_policy;
use super::scenario::house_number_scenario;
use super::value::{house_number, house_number_shape};

// house numbers are precise points; 50m is intentionally tighter than the 100m used for
// streets, which are lines with a broader snap area
const NEAREST_MAX_DISTANCE_IN_METERS: f64 = 50.0;
const REFERENCE_MAX_DISTANCE_FROM_AXIS_IN_METERS: f64 = 100.0;

pub struct house_number_resolution {
  pub number: house_number,
  pub scenario: house_number_scenario,
  pub point: Point<f64>,
}

struct reference {
  value: u32,
  chainage: f64,
}

pub fn place(
  wanted: &house_number,
  known: &[(house_number, Point<f64>)],
  geometry: &Geometry<f64>,
  policy: &house_number_policy,
) -> Option<house_number_resolution> {
  if let Some((_, point)) = known.iter().find(|(number, _)| number == wanted) {
    return Some(house_number_resolution {
      number: wanted.clone(),
      scenario: house_number_scenario::from_osm_data,
      point: *point,
    });
  }
  if wanted.shape() == house_number_shape::compound {
    return None;
  }
  let value = wanted.leading_value()?;
  let axis = street_axis::of(geometry)?;
  let references = references(known, &axis);
  let meters = policy.meters_per_number;
  let (scenario, chainage) = match references.as_slice() {
    [] => (
      house_number_scenario::presumed_from_constants,
      f64::from(value) * meters,
    ),
    [only] => (
      house_number_scenario::presumed_from_one_ref_from_street,
      only.chainage
        + direction_of(only, axis.length(), meters) * (f64::from(value) - f64::from(only.value)) * meters,
    ),
    _ => (
      house_number_scenario::presumed_from_multiple_references_from_street,
      chainage_of_value(value, &references),
    ),
  };
  Some(house_number_resolution {
    number: wanted.clone(),
    scenario,
    point: axis.point_at(chainage),
  })
}

pub fn number_at(
  point: Point<f64>,
  on_street: Point<f64>,
  known: &[(house_number, Point<f64>)],
  geometry: &Geometry<f64>,
  policy: &house_number_policy,
) -> Option<house_number_resolution> {
  if let Some(number) = nearest(point, known) {
    return Some(house_number_resolution {
      number: number.clone(),
      scenario: house_number_scenario::from_osm_data,
      point: on_street,
    });
  }
  let axis = street_axis::of(geometry)?;
  let references = references(known, &axis);
  let chainage = axis.chainage_of(&on_street).along;
  let meters = policy.meters_per_number;
  let (scenario, value) = match references.as_slice() {
    [] => (
      house_number_scenario::presumed_from_constants,
      chainage / meters,
    ),
    [only] => (
      house_number_scenario::presumed_from_one_ref_from_street,
      f64::from(only.value)
        + direction_of(only, axis.length(), meters) * (chainage - only.chainage) / meters,
    ),
    _ => (
      house_number_scenario::presumed_from_multiple_references_from_street,
      value_at_chainage(chainage, &references),
    ),
  };
  Some(house_number_resolution {
    number: house_number::presumed(value.round().max(1.0) as u32),
    scenario,
    point: on_street,
  })
}

fn nearest(point: Point<f64>, known: &[(house_number, Point<f64>)]) -> Option<&house_number> {
  known
    .iter()
    .filter_map(|(number, at)| {
      let distance = point.haversine_distance(at);
      (distance <= NEAREST_MAX_DISTANCE_IN_METERS).then_some((number, distance))
    })
    .min_by(|(_, a), (_, b)| a.total_cmp(b))
    .map(|(number, _)| number)
}

fn references(known: &[(house_number, Point<f64>)], axis: &street_axis) -> Vec<reference> {
  let mut references: Vec<reference> = Vec::new();
  for (number, point) in known {
    if number.shape() == house_number_shape::compound {
      continue;
    }
    let Some(value) = number.leading_value() else {
      continue;
    };
    if references.iter().any(|reference| reference.value == value) {
      continue;
    }
    let at = axis.chainage_of(point);
    if at.off_axis_in_meters <= REFERENCE_MAX_DISTANCE_FROM_AXIS_IN_METERS {
      references.push(reference {
        value,
        chainage: at.along,
      });
    }
  }
  references.sort_by_key(|reference| reference.value);
  references
}

fn factor_of(references: &[reference]) -> f64 {
  let (lowest, highest) = (&references[0], &references[references.len() - 1]);
  (highest.chainage - lowest.chainage) / f64::from(highest.value - lowest.value)
}

fn direction_of(reference: &reference, axis_length: f64, meters_per_number: f64) -> f64 {
  let as_metres = f64::from(reference.value) * meters_per_number;
  let from_start = (reference.chainage - as_metres).abs();
  let from_end = ((axis_length - reference.chainage) - as_metres).abs();
  if from_start <= from_end { 1.0 } else { -1.0 }
}

fn chainage_of_value(value: u32, references: &[reference]) -> f64 {
  let (lowest, highest) = (&references[0], &references[references.len() - 1]);
  if value <= lowest.value {
    return lowest.chainage + (f64::from(value) - f64::from(lowest.value)) * factor_of(references);
  }
  if value >= highest.value {
    return highest.chainage + (f64::from(value) - f64::from(highest.value)) * factor_of(references);
  }
  let above = references.partition_point(|reference| reference.value <= value);
  let (low, high) = (&references[above - 1], &references[above]);
  low.chainage
    + (high.chainage - low.chainage) * f64::from(value - low.value) / f64::from(high.value - low.value)
}

fn value_at_chainage(chainage: f64, references: &[reference]) -> f64 {
  let mut by_chainage: Vec<&reference> = references.iter().collect();
  by_chainage.sort_by(|a, b| a.chainage.total_cmp(&b.chainage));
  let factor = factor_of(references);
  let beyond = |anchor: &reference| {
    if factor == 0.0 {
      f64::from(anchor.value)
    } else {
      f64::from(anchor.value) + (chainage - anchor.chainage) / factor
    }
  };
  let (first, last) = (by_chainage[0], by_chainage[by_chainage.len() - 1]);
  if chainage <= first.chainage {
    return beyond(first);
  }
  if chainage >= last.chainage {
    return beyond(last);
  }
  let above = by_chainage.partition_point(|reference| reference.chainage <= chainage);
  let (low, high) = (by_chainage[above - 1], by_chainage[above]);
  f64::from(low.value)
    + (f64::from(high.value) - f64::from(low.value)) * (chainage - low.chainage)
      / (high.chainage - low.chainage)
}
