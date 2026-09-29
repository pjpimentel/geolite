#![allow(dead_code)]

use super::harness::{output, world};

pub const MAP_COLUMNS: usize = 98;
pub const MAP_ROWS: usize = 24;
const LEVEL: &str = "admin level ";
const ABSENT: &str = "n/a";

// what `geolite tui <path>` prints for the deepest level when stdout is not a terminal: one field
// per line, a blank line, and the map
pub struct leaf {
  pub fields: Vec<(String, String)>,
  pub map: Vec<String>,
}

impl leaf {
  pub fn field(&self, name: &str) -> Option<&str> {
    self
      .fields
      .iter()
      .find(|(field, _)| field == name)
      .map(|(_, value)| value.as_str())
  }

  pub fn names(&self) -> Vec<&str> {
    self.fields.iter().map(|(name, _)| name.as_str()).collect()
  }

  // the fields of the place itself come first, and the lines of its levels after them
  pub fn own_names(&self) -> Vec<&str> {
    let names = self.names();
    let own = names
      .iter()
      .take_while(|name| !name.starts_with(LEVEL))
      .count();
    assert!(
      names[own..].iter().all(|name| name.starts_with(LEVEL)),
      "a field of the place comes after a level: {names:#?}"
    );
    names[..own].to_vec()
  }

  // the lines of the levels, each as the level with what it stands for, `2 (country)`, and its area
  fn levels(&self) -> impl Iterator<Item = (&str, &str)> {
    self
      .fields
      .iter()
      .filter_map(|(name, area)| Some((name.strip_prefix(LEVEL)?, area.as_str())))
  }

  pub fn levels_held(&self) -> Vec<(&str, &str)> {
    self.levels().filter(|(_, area)| *area != ABSENT).collect()
  }

  pub fn levels_absent(&self) -> Vec<&str> {
    self
      .levels()
      .filter(|(_, area)| *area == ABSENT)
      .map(|(level, _)| level)
      .collect()
  }

  // the rows and the columns the shape opened takes on the map, as (first, last)
  pub fn shape_box(&self) -> Option<((usize, usize), (usize, usize))> {
    let drawn: Vec<(usize, usize)> = self
      .map
      .iter()
      .enumerate()
      .flat_map(|(row, line)| {
        line
          .chars()
          .enumerate()
          .filter(|(_, glyph)| is_shape(*glyph))
          .map(move |(column, _)| (row, column))
      })
      .collect();
    let rows = drawn.iter().map(|&(row, _)| row);
    let columns = drawn.iter().map(|&(_, column)| column);
    Some((
      (rows.clone().min()?, rows.max()?),
      (columns.clone().min()?, columns.max()?),
    ))
  }

  pub fn outline_glyphs(&self) -> usize {
    self
      .map
      .iter()
      .flat_map(|line| line.chars())
      .filter(|glyph| is_outline(*glyph))
      .count()
  }
}

// the area around is drawn in braille, the shape opened in half blocks
fn is_outline(glyph: char) -> bool {
  ('\u{2801}'..='\u{28FF}').contains(&glyph)
}

fn is_shape(glyph: char) -> bool {
  matches!(glyph, '▀' | '▄' | '█')
}

pub fn leaf_of(out: &output) -> leaf {
  assert_eq!(out.status, 0, "stderr: {}", out.stderr);
  let (fields, map) = out.stdout.split_once("\n\n").unwrap_or_else(|| {
    panic!(
      "a leaf prints its fields, a blank line and its map:\n{}",
      out.stdout
    )
  });
  leaf {
    fields: fields
      .lines()
      .map(|line| {
        let (name, value) = line
          .split_once("  ")
          .unwrap_or_else(|| panic!("a field is a name and a value: {line:?}"));
        (name.to_string(), value.trim().to_string())
      })
      .collect(),
    map: map.lines().map(str::to_string).collect(),
  }
}

pub fn assert_the_map_fits(leaf: &leaf) {
  assert_eq!(leaf.map.len(), MAP_ROWS, "{:#?}", leaf.map);
  assert!(
    leaf
      .map
      .iter()
      .all(|line| line.chars().count() <= MAP_COLUMNS),
    "{:#?}",
    leaf.map
  );
}

pub fn opened(w: &world, data_path: &std::path::Path, path: &str) -> leaf {
  leaf_of(&w.geolite_in(data_path, &["tui", path]))
}
