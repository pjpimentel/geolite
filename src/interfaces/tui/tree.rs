use rusqlite::Connection;
use std::collections::HashMap;

use super::leaf::leaf;
use crate::admin_level::level;
use crate::admin_level::repository::levels_present;
use crate::admin_level_hierarchy::entity::node;
use crate::admin_level_hierarchy::repository::{children_count_by_parent, children_of, roots};

#[derive(Clone)]
pub enum entry {
  place(node),
  missing { level: level, members: Vec<node> },
}

impl entry {
  pub fn name(&self) -> String {
    match self {
      entry::place(node) => node.name.clone(),
      entry::missing { level, .. } => format!("no {}", level.name()),
    }
  }

  pub fn level(&self) -> level {
    match self {
      entry::place(node) => node.level,
      entry::missing { level, .. } => *level,
    }
  }

  pub fn is_missing(&self) -> bool {
    matches!(self, entry::missing { .. })
  }
}

#[derive(Clone)]
pub struct item {
  pub entry: entry,
  pub inside: usize,
  pub is_folder: bool,
}

impl item {
  // the same place listed under two folders is the same item
  pub fn same_as(&self, other: &item) -> bool {
    match (&self.entry, &other.entry) {
      (entry::place(a), entry::place(b)) => a.id == b.id,
      (entry::missing { level: a, .. }, entry::missing { level: b, .. }) => a == b,
      _ => false,
    }
  }
}

pub fn breadcrumb(path: &[item]) -> String {
  if path.is_empty() {
    return "/".to_string();
  }
  path
    .iter()
    .map(|item| item.entry.name())
    .collect::<Vec<_>>()
    .join(" / ")
}

// a folder entered: the items of the path from a root down to it, and what it lists
pub struct folder {
  pub path: Vec<item>,
  pub items: Vec<item>,
}

impl folder {
  pub fn breadcrumb(&self) -> String {
    breadcrumb(&self.path)
  }

  pub fn current(&self) -> Option<&item> {
    self.path.last()
  }

  pub fn above(&self) -> Option<&item> {
    self.path.len().checked_sub(2).map(|index| &self.path[index])
  }
}

pub enum opened {
  folder(folder),
  leaf(folder, Box<leaf>),
}

pub struct tree {
  ladder: Vec<level>,
  counts: HashMap<i64, usize>,
  pub root_count: usize,
}

pub fn open(conn: &Connection) -> tree {
  let ladder = levels_present(conn);
  let counts = children_count_by_parent(conn);
  let root_count = roots(conn).len();
  tree {
    ladder,
    counts,
    root_count,
  }
}

impl tree {
  pub fn roots(&self, conn: &Connection) -> folder {
    folder {
      path: Vec::new(),
      items: self.grouped(roots(conn), None),
    }
  }

  // a folder is entered; the deepest level of the ladder holds nothing to list, so it is not
  pub fn enter(&self, conn: &Connection, folder: &folder, index: usize) -> Option<folder> {
    let item = folder.items.get(index)?;
    if !item.is_folder {
      return None;
    }
    Some(self.descend(conn, folder, index))
  }

  pub fn parent(&self, conn: &Connection, folder: &folder) -> Option<folder> {
    let mut path = folder.path.clone();
    path.pop()?;
    let items = match path.last() {
      Some(item) => self.listed(conn, item),
      None => self.grouped(roots(conn), None),
    };
    Some(folder { path, items })
  }

  // what a path from a root names, its names separated by `/` and matched without regard to
  // case; the empty path is the roots
  pub fn resolve(&self, conn: &Connection, path: &str) -> Result<opened, String> {
    let mut folder = self.roots(conn);
    let mut segments = path.split('/').map(str::trim).filter(|s| !s.is_empty());
    while let Some(segment) = segments.next() {
      let wanted = segment.to_lowercase();
      let index = folder
        .items
        .iter()
        .position(|item| item.entry.name().to_lowercase() == wanted)
        .ok_or_else(|| format!("no '{segment}' under '{}'", folder.breadcrumb()))?;
      if folder.items[index].is_folder {
        folder = self.descend(conn, &folder, index);
        continue;
      }
      let leaf = super::leaf::open(conn, &folder, index)
        .ok_or_else(|| format!("'{segment}' has no shape to draw"))?;
      return match segments.next() {
        Some(below) => Err(format!("no '{below}' under '{}'", leaf.breadcrumb())),
        None => Ok(opened::leaf(folder, Box::new(leaf))),
      };
    }
    Ok(opened::folder(folder))
  }

  fn descend(&self, conn: &Connection, folder: &folder, index: usize) -> folder {
    let item = folder.items[index].clone();
    let items = self.listed(conn, &item);
    let mut path = folder.path.clone();
    path.push(item);
    folder { path, items }
  }

  fn listed(&self, conn: &Connection, item: &item) -> Vec<item> {
    match &item.entry {
      entry::place(node) => self.grouped(children_of(conn, node.id), Some(node.level)),
      entry::missing { level, members } => self.grouped(members.clone(), Some(*level)),
    }
  }

  // under a folder the ladder expects the next level the database holds: the places deeper than
  // that lack a level, and are kept together first in a folder of their own, where the rule
  // applies again
  fn grouped(&self, nodes: Vec<node>, parent: Option<level>) -> Vec<item> {
    let expected = self
      .ladder
      .iter()
      .copied()
      .find(|&level| parent.is_none_or(|parent| level > parent));
    let (deeper, regular): (Vec<node>, Vec<node>) = nodes
      .into_iter()
      .partition(|node| expected.is_some_and(|expected| node.level > expected));
    let deepest = self.ladder.last().copied();
    let mut items = Vec::with_capacity(regular.len() + 1);
    if let Some(level) = expected
      && !deeper.is_empty()
    {
      items.push(item {
        inside: deeper.len(),
        is_folder: true,
        entry: entry::missing {
          level,
          members: deeper,
        },
      });
    }
    items.extend(regular.into_iter().map(|node| {
      let is_folder = Some(node.level) != deepest;
      item {
        inside: match is_folder {
          true => self.counts.get(&node.id).copied().unwrap_or(0),
          false => 0,
        },
        is_folder,
        entry: entry::place(node),
      }
    }));
    items
  }
}
