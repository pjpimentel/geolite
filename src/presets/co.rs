use crate::admin_level::level;

use super::{DEFAULT, extract_osm_admin_levels_preset, index_addresses_preset, preset};

pub const PRESET: preset = preset {
  iso_3166_1_alpha_2: Some("CO"),
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:es", "name"],
    admin_levels: &[level::country, level::state, level::county, level::city, level::street],
  },
  house_numbers: crate::house_number::house_number_policy {
    drop_values: &["s/n", "sn", "s/nº", "s/no", "s/n.", "s n"],
    shapes: crate::house_number::policy::COMPOUND_SHAPES,
    allow_hash_prefix: true,
    ..DEFAULT.house_numbers
  },
  index_addresses: index_addresses_preset {
    abbreviations: &[
      ("cra.", "carrera"),
      ("kra.", "carrera"),
      ("cr.", "carrera"),
      ("cl.", "calle"),
      ("cll.", "calle"),
      ("av.", "avenida"),
      ("avda.", "avenida"),
      ("dg.", "diagonal"),
      ("tv.", "transversal"),
      ("trans.", "transversal"),
      ("urb.", "urbanización"),
      ("gral.", "general"),
      ("pdte.", "presidente"),
      ("dr.", "doctor"),
      ("sta.", "santa"),
      ("sto.", "santo"),
    ],
    ..DEFAULT.index_addresses
  },
};
