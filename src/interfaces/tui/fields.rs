use std::num::NonZeroU16;

use ratatui::buffer::{Buffer, CellDiffOption, CellWidth};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

const OSM_SITE: &str = "https://www.openstreetmap.org";
const LINK_CLOSE: &str = "\x1b]8;;\x1b\\";
const ABSENT: &str = "n/a";
const GAP: u16 = 2;

#[derive(Clone, Copy)]
pub enum osm_element {
  way(u64),
  relation(u64),
}

impl osm_element {
  pub fn kind(self) -> &'static str {
    match self {
      osm_element::way(_) => "way",
      osm_element::relation(_) => "relation",
    }
  }

  fn id(self) -> u64 {
    match self {
      osm_element::way(id) | osm_element::relation(id) => id,
    }
  }

  fn url(self) -> String {
    format!("{OSM_SITE}/{}/{}", self.kind(), self.id())
  }
}

#[derive(Clone)]
pub enum piece {
  text(String),
  link(osm_element),
  absent,
}

impl piece {
  fn shown(&self) -> String {
    match self {
      piece::text(text) => text.clone(),
      piece::link(element) => element.id().to_string(),
      piece::absent => ABSENT.to_string(),
    }
  }
}

pub fn listed(elements: &[osm_element]) -> Vec<piece> {
  let mut pieces = Vec::with_capacity(elements.len() * 2);
  for element in elements {
    if !pieces.is_empty() {
      pieces.push(piece::text(", ".to_string()));
    }
    pieces.push(piece::link(*element));
  }
  pieces
}

#[derive(Clone)]
pub struct phrase {
  pub pieces: Vec<piece>,
}

impl phrase {
  fn shown(&self) -> String {
    self.pieces.iter().map(piece::shown).collect()
  }

  fn width(&self) -> usize {
    usize::from(self.shown().as_str().cell_width())
  }

  fn words(&self) -> Vec<phrase> {
    let mut words = Vec::new();
    let mut pieces = Vec::new();
    for piece in &self.pieces {
      let piece::text(text) = piece else {
        pieces.push(piece.clone());
        continue;
      };
      let mut fragments = text
        .split(' ')
        .map(|fragment| piece::text(fragment.to_string()));
      pieces.extend(fragments.next());
      for fragment in fragments {
        let word = std::mem::replace(&mut pieces, vec![fragment]);
        words.push(phrase { pieces: word });
      }
    }
    words.push(phrase { pieces });
    words
  }
}

pub struct field {
  pub name: String,
  pub phrases: Vec<phrase>,
}

impl field {
  pub fn of(name: &str, pieces: Vec<piece>) -> field {
    field {
      name: name.to_string(),
      phrases: vec![phrase { pieces }],
    }
  }

  pub fn text(name: &str, value: String) -> field {
    field::of(name, vec![piece::text(value)])
  }

  pub fn shown(&self) -> String {
    let phrases: Vec<String> = self.phrases.iter().map(phrase::shown).collect();
    phrases.join(" ")
  }

  fn lines(&self, width: usize) -> Vec<Vec<phrase>> {
    let units = self.phrases.iter().flat_map(|phrase| {
      if phrase.width() <= width {
        vec![phrase.clone()]
      } else {
        phrase.words()
      }
    });
    let mut lines = Vec::new();
    let mut line: Vec<phrase> = Vec::new();
    let mut taken = 0;
    for unit in units {
      let needed = unit.width();
      if !line.is_empty() && taken + 1 + needed > width {
        lines.push(std::mem::take(&mut line));
        taken = 0;
      }
      taken += needed + usize::from(!line.is_empty());
      line.push(unit);
    }
    lines.push(line);
    lines
  }
}

struct row<'a> {
  name: Option<&'a str>,
  phrases: Vec<phrase>,
}

pub struct sheet<'a> {
  names: u16,
  rows: Vec<row<'a>>,
}

impl sheet<'_> {
  pub fn of(fields: &[field], width: u16) -> sheet<'_> {
    let names = fields
      .iter()
      .map(|field| field.name.len())
      .max()
      .unwrap_or(0) as u16;
    let values = usize::from(width.saturating_sub(names + GAP));
    let rows = fields
      .iter()
      .flat_map(|field| {
        let lines = field.lines(values).into_iter().enumerate();
        lines.map(|(index, phrases)| row {
          name: (index == 0).then_some(field.name.as_str()),
          phrases,
        })
      })
      .collect();
    sheet { names, rows }
  }

  pub fn height(&self) -> u16 {
    self.rows.len() as u16
  }

  pub fn render(&self, scroll: u16, area: Rect, buffer: &mut Buffer) {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let shown = self
      .rows
      .iter()
      .skip(usize::from(scroll))
      .take(usize::from(area.height));
    for (row, y) in shown.zip(area.y..) {
      if let Some(name) = row.name {
        buffer.set_stringn(area.x, y, name, usize::from(area.width), dim);
      }
      let mut x = area.x + self.names + GAP;
      for piece in row.phrases.iter().flat_map(spaced) {
        let room = usize::from(area.right().saturating_sub(x));
        x = match piece {
          piece::text(text) => buffer.set_stringn(x, y, text, room, Style::default()).0,
          piece::link(element) => linked(buffer, (x, y), room, element),
          piece::absent => buffer.set_stringn(x, y, ABSENT, room, dim).0,
        };
      }
    }
  }
}

fn spaced(phrase: &phrase) -> impl Iterator<Item = piece> + '_ {
  let space = piece::text(" ".to_string());
  phrase.pieces.iter().cloned().chain(std::iter::once(space))
}

// each cell carries its whole link, opened and closed, under a forced width of one: ratatui counts
// an escape sequence as text and rewrites only the cells that changed, so a link opened in one cell
// and closed in another is left open by a frame that rewrites only one of the two. the `id` is what
// tells the terminal that the cells side by side are one link
fn linked(buffer: &mut Buffer, (x, y): (u16, u16), room: usize, element: osm_element) -> u16 {
  let underlined = Style::default().add_modifier(Modifier::UNDERLINED);
  let id = element.id();
  let (end, _) = buffer.set_stringn(x, y, id.to_string(), room, underlined);
  let open = format!("\x1b]8;id={}{id};{}\x1b\\", element.kind(), element.url());
  for column in x..end {
    let cell = &mut buffer[(column, y)];
    let symbol = format!("{open}{}{LINK_CLOSE}", cell.symbol());
    cell
      .set_symbol(&symbol)
      .set_diff_option(CellDiffOption::ForcedWidth(NonZeroU16::MIN));
  }
  end
}
