use geo::LineString;
use rusqlite::Connection;
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Range;

use super::entity::hierarchy_edges;
use super::paths::paths_of;
use super::repository;
use crate::admin_level::geometry::{fold_lines, lines_of, merged_way_ids, nearby_pairs};
use crate::admin_level::repository::{self as admin_level_repository, name_row};
use crate::admin_level::{admin_level, admin_level_id, level};
use crate::house_number::repository as house_number_repository;
use crate::progress_report;

const IDS_PER_READ: usize = 30_000;
const REACH_IN_METERS: f64 = 20.0;

pub struct piece {
  name: String,
  post_code: Option<String>,
  members: Vec<i64>,
  parents: Vec<i64>,
  parents_changed: bool,
}

impl piece {
  fn survivor(&self) -> i64 {
    self.members[0]
  }

  fn absorbed(&self) -> &[i64] {
    &self.members[1..]
  }
}

#[derive(Default)]
pub struct fold_report {
  pub pieces: u64,
  pub absorbed: u64,
  pub numbers_moved: u64,
}

fn post_code_of(street: &name_row) -> Option<&str> {
  street
    .post_code
    .as_deref()
    .map(str::trim)
    .filter(|code| !code.is_empty())
}

fn shares_a_parent(a: &[i64], b: &[i64]) -> bool {
  let (mut at_a, mut at_b) = (0, 0);
  while at_a < a.len() && at_b < b.len() {
    match a[at_a].cmp(&b[at_b]) {
      Ordering::Less => at_a += 1,
      Ordering::Greater => at_b += 1,
      Ordering::Equal => return true,
    }
  }
  false
}

fn post_codes_agree(a: Option<&str>, b: Option<&str>) -> bool {
  a.is_none() || b.is_none() || a == b
}

fn union_of_parents<'a>(
  edges: &HashMap<i64, Vec<i64>>,
  members: impl Iterator<Item = &'a [i64]>,
) -> Vec<i64> {
  let union: BTreeSet<i64> = members.flat_map(|parents| parents.iter().copied()).collect();
  if union.len() < 2 {
    return union.into_iter().collect();
  }
  let above: HashSet<i64> = union
    .iter()
    .flat_map(|&parent| paths_of(parent, edges).into_iter().flatten())
    .collect();
  union
    .into_iter()
    .filter(|parent| !above.contains(parent))
    .collect()
}

fn candidate_groups<'a>(
  streets: &'a [name_row],
  edges: &HashMap<i64, Vec<i64>>,
) -> Vec<Vec<&'a name_row>> {
  let mut by_name: HashMap<&str, Vec<&name_row>> = HashMap::new();
  for street in streets.iter().filter(|row| row.admin_level == level::street) {
    if edges
      .get(&street.id)
      .is_some_and(|parents| !parents.is_empty())
    {
      by_name.entry(street.name.as_str()).or_default().push(street);
    }
  }
  let mut groups: Vec<Vec<&name_row>> = by_name
    .into_values()
    .filter(|group| group.len() > 1)
    .collect();
  for group in &mut groups {
    group.sort_by_key(|street| street.id);
  }
  groups.sort_by_key(|group| group[0].id);
  groups
}

struct grown_piece {
  members: Vec<usize>,
  parents: Vec<i64>,
}

fn root_of(root: &mut [usize], mut member: usize) -> usize {
  while root[member] != member {
    root[member] = root[root[member]];
    member = root[member];
  }
  member
}

fn pieces_of(
  lines: &[Vec<LineString<f64>>],
  parents: &[&[i64]],
  codes: &[Option<&str>],
  edges: &HashMap<i64, Vec<i64>>,
) -> Vec<grown_piece> {
  let pairs = nearby_pairs(lines, REACH_IN_METERS);
  let mut root: Vec<usize> = (0..lines.len()).collect();
  let mut root_parents: Vec<Vec<i64>> = parents.iter().map(|parents| parents.to_vec()).collect();
  let mut root_code: Vec<Option<&str>> = codes.to_vec();
  loop {
    let mut grew = false;
    for &(a, b) in &pairs {
      let (root_a, root_b) = (root_of(&mut root, a), root_of(&mut root, b));
      if root_a == root_b
        || !shares_a_parent(&root_parents[root_a], &root_parents[root_b])
        || !post_codes_agree(root_code[root_a], root_code[root_b])
      {
        continue;
      }
      let (kept, gone) = (root_a.min(root_b), root_a.max(root_b));
      let merged = if root_parents[root_a] == root_parents[root_b] {
        root_parents[root_a].clone()
      } else {
        union_of_parents(
          edges,
          [root_parents[root_a].as_slice(), root_parents[root_b].as_slice()].into_iter(),
        )
      };
      root_parents[kept] = merged;
      root_code[kept] = root_code[root_a].or(root_code[root_b]);
      root[gone] = kept;
      grew = true;
    }
    if !grew {
      break;
    }
  }

  let mut piece_of_root: HashMap<usize, usize> = HashMap::new();
  let mut pieces: Vec<grown_piece> = Vec::new();
  for member in 0..lines.len() {
    let member_root = root_of(&mut root, member);
    let piece = *piece_of_root.entry(member_root).or_insert_with(|| {
      pieces.push(grown_piece {
        members: Vec::new(),
        parents: root_parents[member_root].clone(),
      });
      pieces.len() - 1
    });
    pieces[piece].members.push(member);
  }
  pieces
}

fn batches<T>(items: &[T], weight: impl Fn(&T) -> usize) -> Vec<Range<usize>> {
  let mut ranges = Vec::new();
  let mut start = 0;
  let mut load = 0;
  for (at, item) in items.iter().enumerate() {
    let next = weight(item);
    if at > start && load + next > IDS_PER_READ {
      ranges.push(start..at);
      start = at;
      load = 0;
    }
    load += next;
  }
  if start < items.len() {
    ranges.push(start..items.len());
  }
  ranges
}

fn load_lines(conn: &Connection, ids: &[i64]) -> HashMap<i64, Vec<LineString<f64>>> {
  ids
    .chunks(IDS_PER_READ)
    .flat_map(|chunk| admin_level_repository::geometry_by_ids(conn, chunk))
    .map(|(id, geometry)| (id, lines_of(geometry.into_geometry())))
    .collect()
}

fn load_traced_lines(
  conn: &Connection,
  ids: &[i64],
) -> HashMap<i64, Vec<(u64, LineString<f64>)>> {
  let mut folded: HashMap<i64, merged_way_ids> = ids
    .chunks(IDS_PER_READ)
    .flat_map(|chunk| admin_level_repository::merged_way_ids_by_ids(conn, chunk))
    .collect();
  load_lines(conn, ids)
    .into_iter()
    .map(|(id, lines)| {
      let way_ids = folded
        .remove(&id)
        .map(|stored| stored.0)
        .filter(|way_ids| way_ids.len() == lines.len())
        .unwrap_or_else(|| vec![admin_level_id::from_raw(id as u64).osm_id(); lines.len()]);
      (id, way_ids.into_iter().zip(lines).collect())
    })
    .collect()
}

pub fn find(conn: &Connection, progress: impl Fn(progress_report)) -> Vec<piece> {
  let streets = admin_level_repository::load_all_names(conn);
  let edges = repository::load_all_edges(conn);
  let groups = candidate_groups(&streets, &edges);

  let total = groups.len() as u64;
  progress(progress_report {
    total: Some(total),
    processed: 0,
  });
  let mut pieces: Vec<piece> = Vec::new();
  let mut processed: u64 = 0;
  for range in batches(&groups, |group| group.len()) {
    let batch = &groups[range];
    let ids: Vec<i64> = batch.iter().flatten().map(|street| street.id).collect();
    let mut lines = load_lines(conn, &ids);
    for group in batch {
      let member_lines: Vec<Vec<LineString<f64>>> = group
        .iter()
        .map(|street| lines.remove(&street.id).unwrap_or_default())
        .collect();
      let parents: Vec<&[i64]> = group
        .iter()
        .map(|street| edges[&street.id].as_slice())
        .collect();
      let codes: Vec<Option<&str>> = group.iter().map(|&street| post_code_of(street)).collect();
      for grown in pieces_of(&member_lines, &parents, &codes, &edges)
        .into_iter()
        .filter(|grown| grown.members.len() > 1)
      {
        pieces.push(piece {
          name: group[0].name.clone(),
          post_code: grown
            .members
            .iter()
            .find_map(|&member| codes[member])
            .map(str::to_owned),
          parents_changed: grown.parents != parents[grown.members[0]],
          members: grown.members.iter().map(|&member| group[member].id).collect(),
          parents: grown.parents,
        });
      }
    }
    processed += batch.len() as u64;
    progress(progress_report {
      total: Some(total),
      processed,
    });
  }
  pieces
}

pub fn fold(conn: &Connection, pieces: &[piece], progress: impl Fn(progress_report)) -> fold_report {
  let total = pieces.len() as u64;
  progress(progress_report {
    total: Some(total),
    processed: 0,
  });
  let mut report = fold_report::default();
  let mut processed: u64 = 0;
  for range in batches(pieces, |piece| piece.members.len()) {
    let batch = &pieces[range];
    let ids: Vec<i64> = batch
      .iter()
      .flat_map(|piece| piece.members.iter().copied())
      .collect();
    let mut lines = load_traced_lines(conn, &ids);
    let mut survivors: Vec<(admin_level, merged_way_ids)> = Vec::with_capacity(batch.len());
    let mut rewired: Vec<hierarchy_edges> = Vec::new();
    let mut moves: Vec<(i64, i64)> = Vec::new();
    for piece in batch {
      let members = piece
        .members
        .iter()
        .map(|id| lines.remove(id).unwrap_or_default());
      let Some((geometry, way_ids)) = fold_lines(members) else {
        continue;
      };
      survivors.push((
        admin_level {
          id: admin_level_id::from_raw(piece.survivor() as u64),
          level: level::street,
          wkb: geometry.into(),
          name: piece.name.clone(),
          country_iso_code: None,
          post_code: piece.post_code.clone(),
        },
        way_ids,
      ));
      if piece.parents_changed {
        rewired.push(hierarchy_edges {
          admin_level_id: piece.survivor(),
          parents: piece.parents.clone(),
        });
      }
      moves.extend(piece.absorbed().iter().map(|&id| (id, piece.survivor())));
    }

    admin_level_repository::batch_upsert_folded(conn, &survivors);
    repository::replace_parents(conn, &rewired);
    report.numbers_moved += house_number_repository::repoint_streets(conn, &moves) as u64;
    // the numbers move before the rows go: house_numbers.admin_level_id cascades on delete
    let absorbed: Vec<i64> = moves.iter().map(|&(id, _)| id).collect();
    report.absorbed += admin_level_repository::delete_by_ids(conn, &absorbed) as u64;

    report.pieces += survivors.len() as u64;
    processed += batch.len() as u64;
    progress(progress_report {
      total: Some(total),
      processed,
    });
  }
  report
}
