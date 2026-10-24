use crate::admin_level::level;

use super::{extract_osm_admin_levels_preset, index_addresses_preset, preset};

pub const DEFAULT: preset = preset {
  iso_3166_1_alpha_2: None,
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    admin_levels: &[level::country, level::region, level::state, level::district, level::county, level::municipality, level::city, level::locality, level::neighborhood, level::street],
    name_priority: &["name"],
  },
  house_numbers: crate::house_number::house_number_policy {
    drop_values: &[],
    max_digits: 5,
    shapes: crate::house_number::policy::SIMPLE_SHAPES,
    allow_hash_prefix: false,
    meters_per_number: 3.0,
  },
  index_addresses: index_addresses_preset {
    abbreviations: &[],
    boosts: crate::admin_level_hierarchy::tantivy_boosts {
      name_exact: 5.0,
      name_fuzzy: 1.5,
      name_phrase: 10.0,
      name_strict: 3.0,
      name_lower: 2.0,
      hier_exact: 2.0,
      hier_fuzzy: 0.5,
      hier_phrase: 4.0,
      hier_strict: 1.2,
      hier_lower: 0.8,
    },
  },
};
