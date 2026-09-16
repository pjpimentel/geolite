use std::collections::HashMap;

use super::{MAX_PATHS, paths_of};

fn edges(pairs: &[(i64, &[i64])]) -> HashMap<i64, Vec<i64>> {
  pairs
    .iter()
    .map(|(id, parents)| (*id, parents.to_vec()))
    .collect()
}

#[test]
fn _00_an_area_without_parents_has_one_empty_path() {
  assert_eq!(paths_of(1, &edges(&[])), vec![Vec::<i64>::new()]);
  assert_eq!(paths_of(1, &edges(&[(1, &[])])), vec![Vec::<i64>::new()]);
}

#[test]
fn _01_one_parent_gives_one_path_up_to_the_root() {
  let edges = edges(&[(10, &[3]), (3, &[2]), (2, &[1]), (1, &[])]);
  assert_eq!(paths_of(10, &edges), vec![vec![3, 2, 1]]);
  assert_eq!(paths_of(2, &edges), vec![vec![1]]);
}

#[test]
fn _02_two_parents_give_two_paths_in_id_order() {
  let edges = edges(&[(10, &[5, 3]), (3, &[1]), (5, &[1]), (1, &[])]);
  assert_eq!(paths_of(10, &edges), vec![vec![3, 1], vec![5, 1]]);
}

#[test]
fn _03_a_parent_with_two_paths_fans_out_its_children() {
  let edges = edges(&[(10, &[3]), (3, &[4, 2]), (2, &[1]), (4, &[1]), (1, &[])]);
  assert_eq!(paths_of(10, &edges), vec![vec![3, 2, 1], vec![3, 4, 1]]);
}

#[test]
fn _04_a_repeated_parent_counts_once() {
  let edges = edges(&[(10, &[3, 3]), (3, &[])]);
  assert_eq!(paths_of(10, &edges), vec![vec![3]]);
}

#[test]
fn _05_the_paths_are_capped_at_the_maximum_in_order() {
  let parents: Vec<i64> = (1..=40).collect();
  let edges = edges(&[(100, &parents)]);
  let paths = paths_of(100, &edges);
  assert_eq!(paths.len(), MAX_PATHS);
  assert_eq!(paths.first(), Some(&vec![1]));
  assert_eq!(paths.last(), Some(&vec![MAX_PATHS as i64]));
}

#[test]
fn _06_a_cycle_ends_the_climb_instead_of_looping() {
  let edges = edges(&[(1, &[2]), (2, &[1])]);
  let paths = paths_of(1, &edges);
  assert_eq!(paths.len(), 1);
  assert_eq!(paths[0].first(), Some(&2));
}
