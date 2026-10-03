use geo::{BoundingRect, Geometry, Line};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::symbols::Marker;
use ratatui::widgets::Widget;
use ratatui::widgets::canvas::{self, Canvas, Context};

use crate::admin_level::geometry::outline_of;

const TEXT_COLUMNS: u16 = 98;
const TEXT_ROWS: u16 = 24;
const MARGIN: f64 = 0.05;
const CELL_HEIGHT_IN_WIDTHS: f64 = 2.0;
const SCALE_PER_STEP: f64 = 2.0;
const CLOSEST_STEP: i8 = 16;
const METERS_PER_DEGREE: f64 = 111_320.0;

#[derive(Clone, Copy, Default)]
pub struct zoom {
  steps: i8,
}

impl zoom {
  pub fn closer(self) -> zoom {
    zoom {
      steps: (self.steps + 1).min(CLOSEST_STEP),
    }
  }

  pub fn farther(self) -> zoom {
    zoom {
      steps: self.steps.saturating_sub(1),
    }
  }

  fn scale(self) -> f64 {
    SCALE_PER_STEP.powi(i32::from(self.steps))
  }
}

#[derive(Clone, Copy)]
struct extent {
  min_x: f64,
  min_y: f64,
  max_x: f64,
  max_y: f64,
}

impl extent {
  fn of(lines: &[Line<f64>]) -> Option<extent> {
    let mut coords = lines.iter().flat_map(|line| [line.start, line.end]);
    let first = coords.next()?;
    let start = extent {
      min_x: first.x,
      min_y: first.y,
      max_x: first.x,
      max_y: first.y,
    };
    Some(coords.fold(start, |extent, coord| extent {
      min_x: extent.min_x.min(coord.x),
      min_y: extent.min_y.min(coord.y),
      max_x: extent.max_x.max(coord.x),
      max_y: extent.max_y.max(coord.y),
    }))
  }

  fn with(self, other: extent) -> extent {
    extent {
      min_x: self.min_x.min(other.min_x),
      min_y: self.min_y.min(other.min_y),
      max_x: self.max_x.max(other.max_x),
      max_y: self.max_y.max(other.max_y),
    }
  }

  fn centre(self) -> (f64, f64) {
    (
      (self.min_x + self.max_x) / 2.0,
      (self.min_y + self.max_y) / 2.0,
    )
  }

  fn per_cell(self, cells: &cells) -> f64 {
    let across = (self.max_x - self.min_x) / cells.columns;
    let down = (self.max_y - self.min_y) / cells.rows;
    across.max(down).max(f64::EPSILON) * (1.0 + 2.0 * MARGIN)
  }
}

struct shape {
  lines: Vec<Line<f64>>,
  extent: extent,
}

impl shape {
  fn of(geometry: &Geometry<f64>, stretch: f64) -> Option<shape> {
    let lines: Vec<Line<f64>> = outline_of(geometry)
      .into_iter()
      .flat_map(|line| line.lines())
      .map(|line| {
        Line::new(
          (line.start.x * stretch, line.start.y),
          (line.end.x * stretch, line.end.y),
        )
      })
      .collect();
    let extent = extent::of(&lines)?;
    Some(shape { lines, extent })
  }
}

pub struct drawing {
  leaf: shape,
  above: Vec<shape>,
}

impl drawing {
  pub fn of(leaf: &Geometry<f64>, above: &[&Geometry<f64>]) -> Option<drawing> {
    let nearest = above.first().copied().unwrap_or(leaf);
    let stretch = nearest.bounding_rect()?.center().y.to_radians().cos();
    Some(drawing {
      leaf: shape::of(leaf, stretch)?,
      above: above
        .iter()
        .filter_map(|area| shape::of(area, stretch))
        .collect(),
    })
  }

  fn nearest(&self) -> extent {
    self.above.first().unwrap_or(&self.leaf).extent
  }

  fn whole(&self) -> extent {
    self
      .above
      .iter()
      .fold(self.leaf.extent, |whole, area| whole.with(area.extent))
  }
}

struct cells {
  columns: f64,
  rows: f64,
}

impl cells {
  fn of(area: Rect) -> cells {
    cells {
      columns: f64::from(area.width.max(1)),
      rows: f64::from(area.height.max(1)) * CELL_HEIGHT_IN_WIDTHS,
    }
  }
}

struct frame {
  x: [f64; 2],
  y: [f64; 2],
}

fn span(held: f64, half: f64, (min, max): (f64, f64)) -> [f64; 2] {
  let pad = (max - min) * MARGIN;
  let (low, high) = (min - pad + half, max + pad - half);
  let centre = if low >= high { (min + max) / 2.0 } else { held.clamp(low, high) };
  [centre - half, centre + half]
}

impl frame {
  fn around(drawing: &drawing, area: Rect, zoom: zoom) -> frame {
    let cells = cells::of(area);
    let nearest = drawing.nearest();
    let whole = drawing.whole();
    let at_start = nearest.per_cell(&cells);
    let per_cell = (at_start / zoom.scale()).min(whole.per_cell(&cells));
    let scale = at_start / per_cell;
    let (held_x, held_y) = drawing.leaf.extent.centre();
    let (start_x, start_y) = nearest.centre();
    frame {
      x: span(
        held_x + (start_x - held_x) / scale,
        per_cell * cells.columns / 2.0,
        (whole.min_x, whole.max_x),
      ),
      y: span(
        held_y + (start_y - held_y) / scale,
        per_cell * cells.rows / 2.0,
        (whole.min_y, whole.max_y),
      ),
    }
  }

  fn ground_width(&self) -> String {
    let meters = (self.x[1] - self.x[0]) * METERS_PER_DEGREE;
    if meters < 1_000.0 {
      format!("{meters:.0} m")
    } else if meters < 100_000.0 {
      format!("{:.1} km", meters / 1_000.0)
    } else {
      format!("{:.0} km", meters / 1_000.0)
    }
  }
}

pub fn settled(drawing: &drawing, area: Rect, zoom: zoom) -> zoom {
  let cells = cells::of(area);
  let out = drawing.whole().per_cell(&cells) / drawing.nearest().per_cell(&cells);
  let farthest = -(out.log2().ceil().max(0.0) as i8);
  zoom {
    steps: zoom.steps.max(farthest),
  }
}

pub fn ground_width(drawing: &drawing, area: Rect, zoom: zoom) -> String {
  frame::around(drawing, area, zoom).ground_width()
}

fn draw(ctx: &mut Context, shape: &shape, color: Color) {
  for line in &shape.lines {
    ctx.draw(&canvas::Line::new(
      line.start.x,
      line.start.y,
      line.end.x,
      line.end.y,
      color,
    ));
  }
}

pub fn render(drawing: &drawing, zoom: zoom, area: Rect, buffer: &mut Buffer) {
  let frame = frame::around(drawing, area, zoom);
  Canvas::default()
    .marker(Marker::Braille)
    .x_bounds(frame.x)
    .y_bounds(frame.y)
    .paint(|ctx| {
      for area in &drawing.above {
        draw(ctx, area, Color::DarkGray);
      }
      ctx.marker(Marker::HalfBlock);
      draw(ctx, &drawing.leaf, Color::Reset);
    })
    .render(area, buffer);
}

pub fn text(drawing: &drawing) -> Vec<String> {
  let area = Rect::new(0, 0, TEXT_COLUMNS, TEXT_ROWS);
  let mut buffer = Buffer::empty(area);
  render(drawing, zoom::default(), area, &mut buffer);
  (0..area.height)
    .map(|row| {
      let line: String = (0..area.width)
        .map(|column| buffer[(column, row)].symbol())
        .collect();
      line.trim_end().to_string()
    })
    .collect()
}
