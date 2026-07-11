#[derive(Clone, Copy)]
pub struct preset {
  pub name: &'static str,
  pub extract_osm_admin_levels: extract_osm_admin_levels_preset,
  pub extract_house_numbers: extract_house_numbers_preset,
  pub index_user_friendly_name: index_user_friendly_name_preset,
}

#[derive(Clone, Copy)]
pub struct extract_osm_admin_levels_preset {
  pub admin_levels: &'static [u8],
  pub admin_levels_rules: &'static [crate::extract::admin_levels::extraction_rules],
  pub name_priority: &'static [&'static str],
}

#[derive(Clone, Copy)]
pub struct extract_house_numbers_preset {
  pub housenumber_tags: &'static [&'static str],
  pub street_tags: &'static [&'static str],
  pub drop_values: &'static [&'static str],
}

#[derive(Clone, Copy)]
pub struct index_user_friendly_name_preset {
  pub abbreviations: &'static [(&'static str, &'static str)],
  pub boosts: crate::index::admin_levels_hierarchy_tantivy::tantivy_boosts,
}

pub const DEFAULT: preset = preset {
  name: "default",
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    admin_levels: &[2, 3, 4, 5, 6, 7, 8, 9, 10, 12],
    admin_levels_rules: &[],
    name_priority: &["name"],
  },
  extract_house_numbers: extract_house_numbers_preset {
    housenumber_tags: &["addr:housenumber"],
    street_tags: &["addr:street"],
    drop_values: &[],
  },
  index_user_friendly_name: index_user_friendly_name_preset {
    abbreviations: &[],
    boosts: crate::index::admin_levels_hierarchy_tantivy::tantivy_boosts {
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

pub const BRAZIL: preset = preset {
  name: "brazil",
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:pt", "name"],
    admin_levels: &[2, 4, 8, 10, 12],
    ..DEFAULT.extract_osm_admin_levels
  },
  extract_house_numbers: extract_house_numbers_preset {
    drop_values: &["s/n", "sn", "s/nº", "s/no", "s/n.", "s n"],
    ..DEFAULT.extract_house_numbers
  },
  index_user_friendly_name: index_user_friendly_name_preset {
    abbreviations: &[
    ("r.", "rua"),
    ("av.", "avenida"),
    ("pç.", "praça"),
    ("pca.", "praça"),
    ("trav.", "travessa"),
    ("estr.", "estrada"),
    ("rod.", "rodovia"),
    ("al.", "alameda"),
  ],
    ..DEFAULT.index_user_friendly_name
  },
};

pub const PORTUGAL: preset = preset {
  name: "portugal",
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:pt", "name"],
    admin_levels: &[2, 6, 8, 10, 12],
    ..DEFAULT.extract_osm_admin_levels
  },
  extract_house_numbers: extract_house_numbers_preset {
    drop_values: &["s/n", "sn", "s/nº", "s/no", "s/n.", "s n"],
    ..DEFAULT.extract_house_numbers
  },
  index_user_friendly_name: index_user_friendly_name_preset {
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
    ..DEFAULT.index_user_friendly_name
  },
};

pub const ARGENTINA: preset = preset {
  name: "argentina",
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:es", "name"],
    admin_levels: &[2, 4, 5, 8, 10, 12],
    ..DEFAULT.extract_osm_admin_levels
  },
  extract_house_numbers: extract_house_numbers_preset {
    drop_values: &["s/n", "sn", "s/nº", "s/no", "s/n.", "s n"],
    ..DEFAULT.extract_house_numbers
  },
  index_user_friendly_name: index_user_friendly_name_preset {
    abbreviations: &[
    ("av.", "avenida"),
    ("avda.", "avenida"),
    ("pje.", "pasaje"),
    ("diag.", "diagonal"),
    ("bv.", "boulevard"),
    ("blvd.", "boulevard"),
    ("gral.", "general"),
    ("pte.", "presidente"),
    ("cnel.", "coronel"),
    ("dr.", "doctor"),
  ],
    ..DEFAULT.index_user_friendly_name
  },
};

pub const BOLIVIA: preset = preset {
  name: "bolivia",
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:es", "name"],
    admin_levels: &[2, 4, 8, 10, 12],
    ..DEFAULT.extract_osm_admin_levels
  },
  extract_house_numbers: extract_house_numbers_preset {
    drop_values: &["s/n", "sn", "s/nº", "s/no", "s/n.", "s n"],
    ..DEFAULT.extract_house_numbers
  },
  index_user_friendly_name: index_user_friendly_name_preset {
    abbreviations: &[
    ("av.", "avenida"),
    ("avda.", "avenida"),
    ("pje.", "pasaje"),
    ("pza.", "plaza"),
    ("gral.", "general"),
    ("cnel.", "coronel"),
    ("mcal.", "mariscal"),
    ("tte.", "teniente"),
    ("dr.", "doctor"),
  ],
    ..DEFAULT.index_user_friendly_name
  },
};

const ID_PRESET: &[(&str, &preset)] = &[
  ("brazil", &BRAZIL),
  ("centro-oeste", &BRAZIL),
  ("nordeste", &BRAZIL),
  ("norte", &BRAZIL),
  ("sudeste", &BRAZIL),
  ("sul", &BRAZIL),
  ("portugal", &PORTUGAL),
  ("argentina", &ARGENTINA),
  ("bolivia", &BOLIVIA),
];

const INCLUDES_PRESET: &[(&str, &preset)] = &[
  ("brazil", &BRAZIL),
  ("portugal", &PORTUGAL),
  ("argentina", &ARGENTINA),
  ("bolivia", &BOLIVIA),
];

fn lookup_exact(map: &[(&str, &preset)], key: &str) -> Option<preset> {
  map.iter().copied().find(|(k, _)| *k == key).map(|(_, target)| *target)
}

fn lookup_includes(map: &[(&str, &preset)], input: &str) -> Option<preset> {
  map.iter().copied().find(|(k, _)| input.contains(*k)).map(|(_, target)| *target)
}

pub fn resolve(input: Option<String>) -> preset {
  let Some(v) = input.map(|s| s.to_lowercase()) else {
    return DEFAULT;
  };
  lookup_exact(ID_PRESET, &v)
    .or_else(|| lookup_includes(INCLUDES_PRESET, &v))
    .unwrap_or(DEFAULT)
}
