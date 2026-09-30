use geo::{Geometry, HaversineDistance, HaversineLength, LineString, Point};

use crate::admin_level::geometry::{outline_of, projected};

const JOIN_REACH_IN_METERS: f64 = 20.0;
const HEADING_SAMPLE_IN_METERS: f64 = 30.0;
const MAX_TURN_IN_DEGREES: f64 = 90.0;

pub struct chainage {
  pub along: f64,
  pub off_axis_in_meters: f64,
}

pub struct street_axis {
  vertices: Vec<Point<f64>>,
  chainages: Vec<f64>,
}

impl street_axis {
  pub fn of(geometry: &Geometry<f64>) -> Option<Self> {
    let lines: Vec<&LineString<f64>> = outline_of(geometry)
      .into_iter()
      .filter(|line| line.0.len() >= 2)
      .collect();
    if lines.is_empty() {
      return None;
    }
    let vertices = join(&lines);
    let mut chainages = Vec::with_capacity(vertices.len());
    let mut along = 0.0;
    for (index, vertex) in vertices.iter().enumerate() {
      if index > 0 {
        along += vertices[index - 1].haversine_distance(vertex);
      }
      chainages.push(along);
    }
    Some(Self {
      vertices,
      chainages,
    })
  }

  pub fn length(&self) -> f64 {
    self.chainages.last().copied().unwrap_or(0.0)
  }

  pub fn chainage_of(&self, point: &Point<f64>) -> chainage {
    let mut nearest = chainage {
      along: 0.0,
      off_axis_in_meters: f64::INFINITY,
    };
    for (index, pair) in self.vertices.windows(2).enumerate() {
      let fraction = fraction_along(pair[0], pair[1], point);
      let off_axis_in_meters = point.haversine_distance(&between(pair[0], pair[1], fraction));
      if off_axis_in_meters < nearest.off_axis_in_meters {
        let (from, to) = (self.chainages[index], self.chainages[index + 1]);
        nearest = chainage {
          along: from + (to - from) * fraction,
          off_axis_in_meters,
        };
      }
    }
    nearest
  }

  pub fn point_at(&self, along: f64) -> Point<f64> {
    let along = along.clamp(0.0, self.length());
    let index = self
      .chainages
      .partition_point(|&chainage| chainage <= along)
      .saturating_sub(1)
      .min(self.vertices.len() - 2);
    let (from, to) = (self.chainages[index], self.chainages[index + 1]);
    let fraction = if to > from { (along - from) / (to - from) } else { 0.0 };
    between(self.vertices[index], self.vertices[index + 1], fraction)
  }
}

fn between(a: Point<f64>, b: Point<f64>, fraction: f64) -> Point<f64> {
  Point::new(
    a.x() + (b.x() - a.x()) * fraction,
    a.y() + (b.y() - a.y()) * fraction,
  )
}

fn fraction_along(a: Point<f64>, b: Point<f64>, point: &Point<f64>) -> f64 {
  let (a, b, p) = (projected(&a.0), projected(&b.0), projected(&point.0));
  let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
  let squared = dx * dx + dy * dy;
  if squared == 0.0 {
    return 0.0;
  }
  (((p.x() - a.x()) * dx + (p.y() - a.y()) * dy) / squared).clamp(0.0, 1.0)
}

struct joint {
  line: usize,
  reversed: bool,
  at_tail: bool,
  turn: f64,
  gap: f64,
}

impl joint {
  fn tighter_than(&self, other: &Self) -> bool {
    (self.turn, self.gap) < (other.turn, other.gap)
  }
}

fn join(lines: &[&LineString<f64>]) -> Vec<Point<f64>> {
  let mut remaining: Vec<Vec<Point<f64>>> =
    lines.iter().map(|line| line.points().collect()).collect();
  let seed = (0..remaining.len())
    .max_by(|&a, &b| {
      lines[a]
        .haversine_length()
        .total_cmp(&lines[b].haversine_length())
        .then(b.cmp(&a))
    })
    .unwrap_or(0);
  let mut axis = remaining.remove(seed);
  while let Some(joint) = tightest_joint(&axis, &remaining) {
    let mut line = remaining.remove(joint.line);
    if joint.reversed {
      line.reverse();
    }
    if joint.at_tail {
      axis.extend(line);
    } else {
      line.extend(axis);
      axis = line;
    }
  }
  axis
}

fn tightest_joint(axis: &[Point<f64>], remaining: &[Vec<Point<f64>>]) -> Option<joint> {
  let mut tightest: Option<joint> = None;
  for (index, line) in remaining.iter().enumerate() {
    for reversed in [false, true] {
      let oriented: Vec<Point<f64>> = if reversed {
        line.iter().rev().copied().collect()
      } else {
        line.clone()
      };
      let candidates = [
        joint {
          line: index,
          reversed,
          at_tail: true,
          turn: turn_between(heading_out(axis), heading_in(&oriented)),
          gap: axis[axis.len() - 1].haversine_distance(&oriented[0]),
        },
        joint {
          line: index,
          reversed,
          at_tail: false,
          turn: turn_between(heading_out(&oriented), heading_in(axis)),
          gap: oriented[oriented.len() - 1].haversine_distance(&axis[0]),
        },
      ];
      for candidate in candidates {
        if candidate.gap <= JOIN_REACH_IN_METERS
          && candidate.turn <= MAX_TURN_IN_DEGREES
          && tightest
            .as_ref()
            .is_none_or(|best| candidate.tighter_than(best))
        {
          tightest = Some(candidate);
        }
      }
    }
  }
  tightest
}

fn heading_out(points: &[Point<f64>]) -> (f64, f64) {
  let mut walked = 0.0;
  let mut index = points.len() - 1;
  while index > 0 && walked < HEADING_SAMPLE_IN_METERS {
    walked += points[index - 1].haversine_distance(&points[index]);
    index -= 1;
  }
  vector(points[index], points[points.len() - 1])
}

fn heading_in(points: &[Point<f64>]) -> (f64, f64) {
  let mut walked = 0.0;
  let mut index = 0;
  while index < points.len() - 1 && walked < HEADING_SAMPLE_IN_METERS {
    walked += points[index].haversine_distance(&points[index + 1]);
    index += 1;
  }
  vector(points[0], points[index])
}

fn vector(from: Point<f64>, to: Point<f64>) -> (f64, f64) {
  let (from, to) = (projected(&from.0), projected(&to.0));
  (to.x() - from.x(), to.y() - from.y())
}

fn turn_between(a: (f64, f64), b: (f64, f64)) -> f64 {
  let (length_a, length_b) = (a.0.hypot(a.1), b.0.hypot(b.1));
  if length_a == 0.0 || length_b == 0.0 {
    return 0.0;
  }
  ((a.0 * b.0 + a.1 * b.1) / (length_a * length_b))
    .clamp(-1.0, 1.0)
    .acos()
    .to_degrees()
}
