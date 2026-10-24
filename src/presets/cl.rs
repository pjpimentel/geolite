use crate::admin_level::level;

use super::{DEFAULT, extract_osm_admin_levels_preset, index_addresses_preset, preset};

pub const PRESET: preset = preset {
  iso_3166_1_alpha_2: Some("CL"),
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:es", "name"],
    admin_levels: &[level::country, level::state, level::city, level::street],
  },
  house_numbers: crate::house_number::house_number_policy {
    drop_values: &["s/n", "sn", "s/nº", "s/no", "s/n.", "s n"],
    ..DEFAULT.house_numbers
  },
  index_addresses: index_addresses_preset {
    abbreviations: &[
      ("av.", "avenida"),
      ("avda.", "avenida"),
      ("pje.", "pasaje"),
      ("psje.", "pasaje"),
      ("cjon.", "callejón"),
      ("cno.", "camino"),
      ("pza.", "plaza"),
      ("pob.", "población"),
      ("gral.", "general"),
      ("cnel.", "coronel"),
      ("tte.", "teniente"),
      ("almte.", "almirante"),
      ("pdte.", "presidente"),
      ("dr.", "doctor"),
      ("sta.", "santa"),
      ("sto.", "santo"),
    ],
    ..DEFAULT.index_addresses
  },
};
