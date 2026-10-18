use crate::admin_level::level;

use super::{DEFAULT, extract_osm_admin_levels_preset, index_addresses_preset, preset};

pub const PRESET: preset = preset {
  iso_3166_1_alpha_2: Some("PT"),
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:pt", "name"],
    admin_levels: &[level::country, level::county, level::city, level::neighborhood, level::street],
  },
  house_numbers: crate::house_number::house_number_policy {
    drop_values: &["s/n", "sn", "s/nº", "s/no", "s/n.", "s n"],
    ..DEFAULT.house_numbers
  },
  index_addresses: index_addresses_preset {
    abbreviations: &[
      ("r.", "rua"),
      ("av.", "avenida"),
      ("pç.", "praça"),
      ("pca.", "praça"),
      ("trav.", "travessa"),
      ("tv.", "travessa"),
      ("estr.", "estrada"),
      ("al.", "alameda"),
      ("cç.", "calçada"),
      ("lg.", "largo"),
      ("pct.", "praceta"),
      ("rot.", "rotunda"),
      ("bc.", "beco"),
      ("urb.", "urbanização"),
      ("bº", "bairro"),
    ],
    ..DEFAULT.index_addresses
  },
};
