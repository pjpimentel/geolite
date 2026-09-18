use std::collections::HashMap;

pub const MAX_PATHS: usize = 32;
// deeper than the scale allows: a chain still climbing past this is a cycle in the edges
const MAX_DEPTH: usize = 32;

// every path from an area up to a root, most specific first and without the area itself: the
// parents in id order and, under each parent, its own paths in the order it enumerates them, so
// that a position in the list is the same ordinal for the build of the index and for the query
pub fn paths_of(id: i64, edges: &HashMap<i64, Vec<i64>>) -> Vec<Vec<i64>> {
  let mut paths = climb(id, edges, 0);
  if paths.len() > MAX_PATHS {
    crate::debug!(
      "debug: area {id} has {} paths, keeping the first {MAX_PATHS}",
      paths.len()
    );
    paths.truncate(MAX_PATHS);
  }
  paths
}

fn climb(id: i64, edges: &HashMap<i64, Vec<i64>>, depth: usize) -> Vec<Vec<i64>> {
  let mut parents: Vec<i64> = edges.get(&id).cloned().unwrap_or_default();
  parents.sort_unstable();
  parents.dedup();
  if parents.is_empty() || depth >= MAX_DEPTH {
    return vec![vec![]];
  }
  let mut paths: Vec<Vec<i64>> = Vec::new();
  for parent in parents {
    for tail in climb(parent, edges, depth + 1) {
      let mut path: Vec<i64> = Vec::with_capacity(tail.len() + 1);
      path.push(parent);
      path.extend(tail);
      paths.push(path);
    }
  }
  paths
}
