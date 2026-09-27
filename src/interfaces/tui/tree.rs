use rusqlite::Connection;
use std::collections::HashMap;

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

  pub fn is_folder(&self) -> bool {
    self.level() != level::street
  }

  pub fn is_missing(&self) -> bool {
    matches!(self, entry::missing { .. })
  }
}

#[derive(Clone)]
pub struct item {
  pub entry: entry,
  pub inside: usize,
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

// a folder entered: the items of the path from a root down to it, and what it lists
pub struct folder {
  pub path: Vec<item>,
  pub items: Vec<item>,
}

impl folder {
  pub fn breadcrumb(&self) -> String {
    if self.path.is_empty() {
      return "/".to_string();
    }
    self
      .path
      .iter()
      .map(|item| item.entry.name())
      .collect::<Vec<_>>()
      .join(" / ")
  }

  pub fn current(&self) -> Option<&item> {
    self.path.last()
  }

  pub fn above(&self) -> Option<&item> {
    self.path.len().checked_sub(2).map(|index| &self.path[index])
  }
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

  // a folder is entered; a street never contains anything, so it is not
  pub fn enter(&self, conn: &Connection, folder: &folder, index: usize) -> Option<folder> {
    let item = folder.items.get(index)?;
    if !item.entry.is_folder() {
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

  // the folder named by a path from a root, its names separated by `/` and matched without
  // regard to case; the empty path is the roots
  pub fn resolve(&self, conn: &Connection, path: &str) -> Result<folder, String> {
    let mut folder = self.roots(conn);
    for segment in path.split('/').map(str::trim).filter(|s| !s.is_empty()) {
      let wanted = segment.to_lowercase();
      let index = folder
        .items
        .iter()
        .position(|item| item.entry.name().to_lowercase() == wanted)
        .ok_or_else(|| format!("no '{segment}' under '{}'", folder.breadcrumb()))?;
      folder = self.descend(conn, &folder, index);
    }
    Ok(folder)
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
    let mut items = Vec::with_capacity(regular.len() + 1);
    if let Some(level) = expected
      && !deeper.is_empty()
    {
      items.push(item {
        inside: deeper.len(),
        entry: entry::missing {
          level,
          members: deeper,
        },
      });
    }
    items.extend(regular.into_iter().map(|node| item {
      inside: match node.level {
        level::street => 0,
        _ => self.counts.get(&node.id).copied().unwrap_or(0),
      },
      entry: entry::place(node),
    }));
    items
  }
}
