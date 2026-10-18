use crate::admin_level::level;

use super::{DEFAULT, extract_osm_admin_levels_preset, index_addresses_preset, preset};

pub const PRESET: preset = preset {
  iso_3166_1_alpha_2: Some("NL"),
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:nl", "name"],
    admin_levels: &[level::country, level::state, level::city, level::street],
  },
  house_numbers: DEFAULT.house_numbers,
  index_addresses: index_addresses_preset {
    abbreviations: &[
      ("str.", "straat"),
      ("ln.", "laan"),
      ("pl.", "plein"),
      ("st.", "sint"),
      ("burg.", "burgemeester"),
      ("prof.", "professor"),
      ("dr.", "doctor"),
      ("mr.", "meester"),
      ("kon.", "koningin"),
    ],
    ..DEFAULT.index_addresses
  },
};
