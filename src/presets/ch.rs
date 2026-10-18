use crate::admin_level::level;

use super::{DEFAULT, extract_osm_admin_levels_preset, index_addresses_preset, preset};

pub const PRESET: preset = preset {
  iso_3166_1_alpha_2: Some("CH"),
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    admin_levels: &[level::country, level::state, level::city, level::street],
    ..DEFAULT.extract_osm_admin_levels
  },
  house_numbers: DEFAULT.house_numbers,
  index_addresses: index_addresses_preset {
    abbreviations: &[
      ("str.", "strasse"),
      ("av.", "avenue"),
      ("ch.", "chemin"),
      ("rte.", "route"),
      ("pl.", "place"),
      ("bd.", "boulevard"),
      ("imp.", "impasse"),
      ("st.", "sankt"),
    ],
    ..DEFAULT.index_addresses
  },
};
