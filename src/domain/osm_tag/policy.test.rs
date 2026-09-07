use super::tag_policy;

const STRINGS: &[&str] = &["", "highway", "residential", "name", "Rua Alfa", "building"];

fn policy(include: Option<&[&str]>, ignore: Option<&[&str]>) -> tag_policy {
  tag_policy {
    include: include.map(|l| l.iter().map(ToString::to_string).collect()),
    ignore: ignore.map(|l| l.iter().map(ToString::to_string).collect()),
  }
}

#[test]
fn _00_an_empty_policy_keeps_every_tag() {
  let policy = tag_policy::default();
  assert!(policy.passes("highway"));
  assert!(policy.passes("anything"));
}

#[test]
fn _01_an_include_list_keeps_only_what_it_names() {
  let policy = policy(Some(&["name", "highway"]), None);
  assert!(policy.passes("name"));
  assert!(policy.passes("highway"));
  assert!(!policy.passes("building"));
}

#[test]
fn _02_an_ignore_list_drops_only_what_it_names() {
  let policy = policy(None, Some(&["building"]));
  assert!(policy.passes("name"));
  assert!(!policy.passes("building"));
}

#[test]
fn _03_the_two_lists_apply_together() {
  let policy = policy(Some(&["name", "building"]), Some(&["building"]));
  assert!(policy.passes("name"));
  assert!(!policy.passes("building"), "ignored even though it is included");
  assert!(!policy.passes("highway"), "not on the include list");
}

#[test]
fn _04_filter_resolves_indexes_against_the_string_table() {
  let policy = tag_policy::default();
  let out = policy.filter(STRINGS, &[1, 3], &[2, 4]);
  assert_eq!(out, vec![("highway", "residential"), ("name", "Rua Alfa")]);
}

#[test]
fn _05_filter_drops_the_pairs_the_policy_rejects() {
  let policy = policy(None, Some(&["highway"]));
  let out = policy.filter(STRINGS, &[1, 3], &[2, 4]);
  assert_eq!(out, vec![("name", "Rua Alfa")]);
}

#[test]
fn _06_filter_skips_pairs_pointing_outside_the_string_table() {
  let policy = tag_policy::default();
  let out = policy.filter(STRINGS, &[1, 99], &[2, 4]);
  assert_eq!(out, vec![("highway", "residential")]);
}
