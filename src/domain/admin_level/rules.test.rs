use super::super::scale::level;
use super::{extraction_rules, resolve_rules};
use crate::database::osm_ways::filters;

// 00: nivel sem override e sem default retorna listas vazias
#[test]
fn _00_level_without_rules_resolves_to_empty_lists() {
  let (include, exclude) = resolve_rules(level::city, &[]);
  assert!(include.is_empty());
  assert!(exclude.is_empty());
}

// 01: nivel 10 sem override cai no default_include (place=neighbourhood + place=suburb)
#[test]
fn _01_level_10_resolves_to_default_include() {
  let (include, exclude) = resolve_rules(level::neighborhood, &[]);
  assert_eq!(include.len(), 2);
  assert!(matches!(include[0], filters::include_place_neighbourhood));
  assert!(matches!(include[1], filters::include_place_suburb));
  assert!(exclude.is_empty());
}

// 02: nivel 12 sem override cai no default_exclude (5 filtros de exclusao)
#[test]
fn _02_level_12_resolves_to_default_exclude() {
  let (include, exclude) = resolve_rules(level::street, &[]);
  assert!(include.is_empty());
  assert_eq!(exclude.len(), 5);
  assert!(matches!(exclude[0], filters::exclude_place_neighbourhood));
  assert!(matches!(exclude[4], filters::exclude_waterway));
}

// 03: override do mesmo nivel tem precedencia sobre o default
#[test]
fn _03_override_takes_precedence_over_default() {
  const INCLUDE: &[filters] = &[filters::include_highway_primary];
  const EXCLUDE: &[filters] = &[filters::exclude_building];
  let overrides = [extraction_rules {
    level: level::street,
    include: INCLUDE,
    exclude: EXCLUDE,
  }];

  let (include, exclude) = resolve_rules(level::street, &overrides);
  assert_eq!(include.len(), 1);
  assert!(matches!(include[0], filters::include_highway_primary));
  assert_eq!(exclude.len(), 1);
}

// 04: override de outro nivel nao afeta o nivel consultado
#[test]
fn _04_override_for_another_level_is_ignored() {
  const INCLUDE: &[filters] = &[filters::include_highway_primary];
  let overrides = [extraction_rules {
    level: level::state,
    include: INCLUDE,
    exclude: &[],
  }];

  let (include, exclude) = resolve_rules(level::street, &overrides);
  assert!(include.is_empty(), "nivel 12 nao deve herdar override do 4");
  assert_eq!(exclude.len(), 5, "nivel 12 mantem o default_exclude");
}
