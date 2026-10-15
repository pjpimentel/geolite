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
  base { name: String },
}

impl entry {
  pub fn name(&self) -> String {
    match self {
      entry::place(node) => node.name.clone(),
      entry::missing { level, .. } => format!("no {}", level.name()),
      entry::base { name } => name.clone(),
    }
  }

  pub fn level_name(&self) -> &'static str {
    match self {
      entry::place(node) => node.level.name(),
      entry::missing { level, .. } => level.name(),
      entry::base { .. } => "base",
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
  pub base: usize,
}

impl item {
  // the same place listed under two folders is the same item
  pub fn same_as(&self, other: &item) -> bool {
    self.base == other.base
      && match (&self.entry, &other.entry) {
        (entry::place(a), entry::place(b)) => a.id == b.id,
        (entry::missing { level: a, .. }, entry::missing { level: b, .. }) => a == b,
        (entry::base { .. }, entry::base { .. }) => true,
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

struct opened_base {
  name: String,
  conn: Connection,
  ladder: Vec<level>,
  counts: HashMap<i64, usize>,
  root_count: usize,
}

// the bases of the data path, each its own tree; one base is listed from its roots, several from
// a folder per base, named after its country
pub struct tree {
  bases: Vec<opened_base>,
  pub root_count: usize,
}

pub fn open(bases: Vec<(String, Connection)>) -> tree {
  let bases: Vec<opened_base> = bases
    .into_iter()
    .map(|(name, conn)| opened_base {
      ladder: levels_present(&conn),
      counts: children_count_by_parent(&conn),
      root_count: roots(&conn).len(),
      name,
      conn,
    })
    .collect();
  let root_count = match bases.as_slice() {
    [only] => only.root_count,
    many => many.len(),
  };
  tree { bases, root_count }
}

impl tree {
  pub fn conn_of(&self, item: &item) -> &Connection {
    &self.bases[item.base].conn
  }

  pub fn roots(&self) -> folder {
    let items = match self.bases.as_slice() {
      [only] => self.grouped(0, roots(&only.conn), None),
      many => many
        .iter()
        .enumerate()
        .map(|(base, opened)| item {
          entry: entry::base {
            name: opened.name.clone(),
          },
          inside: opened.root_count,
          is_folder: true,
          base,
        })
        .collect(),
    };
    folder {
      path: Vec::new(),
      items,
    }
  }

  // a folder is entered; the deepest level of the ladder holds nothing to list, so it is not
  pub fn enter(&self, folder: &folder, index: usize) -> Option<folder> {
    let item = folder.items.get(index)?;
    if !item.is_folder {
      return None;
    }
    Some(self.descend(folder, index))
  }

  pub fn parent(&self, folder: &folder) -> Option<folder> {
    let mut path = folder.path.clone();
    path.pop()?;
    let items = match path.last() {
      Some(item) => self.listed(item),
      None => self.roots().items,
    };
    Some(folder { path, items })
  }

  // what a path from a root names, its names separated by `/` and matched without regard to
  // case; the empty path is the roots
  pub fn resolve(&self, path: &str) -> Result<opened, String> {
    let mut folder = self.roots();
    let mut segments = path.split('/').map(str::trim).filter(|s| !s.is_empty());
    while let Some(segment) = segments.next() {
      let wanted = segment.to_lowercase();
      let index = folder
        .items
        .iter()
        .position(|item| item.entry.name().to_lowercase() == wanted)
        .ok_or_else(|| format!("no '{segment}' under '{}'", folder.breadcrumb()))?;
      if folder.items[index].is_folder {
        folder = self.descend(&folder, index);
        continue;
      }
      let conn = self.conn_of(&folder.items[index]);
      let leaf = super::leaf::open(conn, &folder, index)
        .ok_or_else(|| format!("'{segment}' has no shape to draw"))?;
      return match segments.next() {
        Some(below) => Err(format!("no '{below}' under '{}'", leaf.breadcrumb())),
        None => Ok(opened::leaf(folder, Box::new(leaf))),
      };
    }
    Ok(opened::folder(folder))
  }

  fn descend(&self, folder: &folder, index: usize) -> folder {
    let item = folder.items[index].clone();
    let items = self.listed(&item);
    let mut path = folder.path.clone();
    path.push(item);
    folder { path, items }
  }

  fn listed(&self, item: &item) -> Vec<item> {
    let conn = self.conn_of(item);
    match &item.entry {
      entry::place(node) => self.grouped(item.base, children_of(conn, node.id), Some(node.level)),
      entry::missing { level, members } => self.grouped(item.base, members.clone(), Some(*level)),
      entry::base { .. } => self.grouped(item.base, roots(conn), None),
    }
  }

  // under a folder the ladder expects the next level the base holds: the places deeper than
  // that lack a level, and are kept together first in a folder of their own, where the rule
  // applies again
  fn grouped(&self, base: usize, nodes: Vec<node>, parent: Option<level>) -> Vec<item> {
    let opened = &self.bases[base];
    let expected = opened
      .ladder
      .iter()
      .copied()
      .find(|&level| parent.is_none_or(|parent| level > parent));
    let (deeper, regular): (Vec<node>, Vec<node>) = nodes
      .into_iter()
      .partition(|node| expected.is_some_and(|expected| node.level > expected));
    let deepest = opened.ladder.last().copied();
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
        base,
      });
    }
    items.extend(regular.into_iter().map(|node| {
      let is_folder = Some(node.level) != deepest;
      item {
        inside: match is_folder {
          true => opened.counts.get(&node.id).copied().unwrap_or(0),
          false => 0,
        },
        is_folder,
        entry: entry::place(node),
        base,
      }
    }));
    items
  }
}
