use super::{admin_level_id, osm_element_kind};

#[test]
fn _00_way_ids_pack_to_even_values() {
  let actual: Vec<u64> = (0u64..=10).map(|n| admin_level_id::from_way(n).raw()).collect();
  assert_eq!(actual, vec![0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20]);
}

#[test]
fn _01_relation_ids_pack_to_odd_values() {
  let actual: Vec<u64> = (0u64..=10)
    .map(|n| admin_level_id::from_relation(n).raw())
    .collect();
  assert_eq!(actual, vec![1, 3, 5, 7, 9, 11, 13, 15, 17, 19, 21]);
}

#[test]
fn _02_raw_values_unpack_to_kind_and_osm_id() {
  let actual: Vec<(osm_element_kind, u64)> = (0u64..=11)
    .map(admin_level_id::from_raw)
    .map(|id| (id.kind(), id.osm_id()))
    .collect();
  assert_eq!(
    actual,
    vec![
      (osm_element_kind::way, 0),
      (osm_element_kind::relation, 0),
      (osm_element_kind::way, 1),
      (osm_element_kind::relation, 1),
      (osm_element_kind::way, 2),
      (osm_element_kind::relation, 2),
      (osm_element_kind::way, 3),
      (osm_element_kind::relation, 3),
      (osm_element_kind::way, 4),
      (osm_element_kind::relation, 4),
      (osm_element_kind::way, 5),
      (osm_element_kind::relation, 5),
    ],
  );
}

#[test]
fn _03_way_and_relation_sharing_an_osm_id_stay_distinct() {
  assert_ne!(admin_level_id::from_way(42), admin_level_id::from_relation(42));
}

#[test]
fn _04_packing_round_trips_for_both_kinds() {
  for n in [0u64, 1, 2, 12345, 9_000_000_000] {
    let way = admin_level_id::from_way(n);
    assert_eq!(way.kind(), osm_element_kind::way);
    assert_eq!(way.osm_id(), n);

    let relation = admin_level_id::from_relation(n);
    assert_eq!(relation.kind(), osm_element_kind::relation);
    assert_eq!(relation.osm_id(), n);
  }
}
