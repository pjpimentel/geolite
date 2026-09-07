use super::level;

const ON_THE_SCALE: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 14, 30];

#[test]
fn _00_new_accepts_every_level_on_the_scale() {
  for value in ON_THE_SCALE {
    let level = level::new(value).unwrap_or_else(|| panic!("level {value} must be on the scale"));
    assert_eq!(level.value(), value);
  }
}

#[test]
fn _01_new_rejects_values_off_the_scale() {
  for value in [0u8, 11, 13, 15, 29, 31, 255] {
    assert_eq!(level::new(value), None, "level {value} must be off the scale");
  }
}

#[test]
fn _02_every_level_has_a_name() {
  let named: Vec<(u8, &str)> = ON_THE_SCALE
    .into_iter()
    .map(|value| (value, level::new(value).unwrap().name()))
    .collect();
  assert_eq!(
    named,
    vec![
      (1, "continent"),
      (2, "country"),
      (3, "region"),
      (4, "state"),
      (5, "district"),
      (6, "county"),
      (7, "municipality"),
      (8, "city"),
      (9, "locality"),
      (10, "neighborhood"),
      (12, "street"),
      (14, "address"),
      (30, "house_number"),
    ]
  );
}

#[test]
fn _03_levels_order_by_value() {
  assert!(level::country < level::state);
  assert!(level::street < level::house_number);
  let mut unsorted = vec![level::street, level::country, level::city, level::neighborhood];
  unsorted.sort();
  assert_eq!(
    unsorted,
    vec![level::country, level::city, level::neighborhood, level::street]
  );
}
