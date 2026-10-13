use crate::admin_level::level;

#[derive(Clone, Copy)]
pub struct country {
  pub iso_3166_1_alpha_2: &'static str,
  pub boundary_wkt: &'static str,
}

#[derive(Clone, Copy)]
pub struct preset {
  pub country: Option<country>,
  pub extract_osm_admin_levels: extract_osm_admin_levels_preset,
  pub house_numbers: crate::house_number::house_number_policy,
  pub index_addresses: index_addresses_preset,
}

#[derive(Clone, Copy)]
pub struct extract_osm_admin_levels_preset {
  pub admin_levels: &'static [level],
  pub name_priority: &'static [&'static str],
}

#[derive(Clone, Copy)]
pub struct index_addresses_preset {
  pub abbreviations: &'static [(&'static str, &'static str)],
  pub boosts: crate::admin_level_hierarchy::tantivy_boosts,
}

pub const DEFAULT: preset = preset {
  country: None,
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

pub const BRAZIL: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "BR",
    boundary_wkt: include_str!("boundaries/br.wkt"),
  }),
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:pt", "name"],
    admin_levels: &[level::country, level::state, level::city, level::neighborhood, level::street],
  },
  house_numbers: crate::house_number::house_number_policy {
    drop_values: &["s/n", "sn", "s/nº", "s/no", "s/n.", "s n"],
    meters_per_number: 1.0,
    ..DEFAULT.house_numbers
  },
  index_addresses: index_addresses_preset {
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
    ..DEFAULT.index_addresses
  },
};

pub const PORTUGAL: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "PT",
    boundary_wkt: include_str!("boundaries/pt.wkt"),
  }),
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

pub const ARGENTINA: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "AR",
    boundary_wkt: include_str!("boundaries/ar.wkt"),
  }),
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:es", "name"],
    admin_levels: &[level::country, level::state, level::district, level::city, level::neighborhood, level::street],
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
    ("diag.", "diagonal"),
    ("bv.", "boulevard"),
    ("blvd.", "boulevard"),
    ("gral.", "general"),
    ("pte.", "presidente"),
    ("cnel.", "coronel"),
    ("dr.", "doctor"),
  ],
    ..DEFAULT.index_addresses
  },
};

pub const BOLIVIA: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "BO",
    boundary_wkt: include_str!("boundaries/bo.wkt"),
  }),
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:es", "name"],
    admin_levels: &[level::country, level::state, level::city, level::neighborhood, level::street],
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
    ("pza.", "plaza"),
    ("gral.", "general"),
    ("cnel.", "coronel"),
    ("mcal.", "mariscal"),
    ("tte.", "teniente"),
    ("dr.", "doctor"),
  ],
    ..DEFAULT.index_addresses
  },
};

pub const CHILE: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "CL",
    boundary_wkt: include_str!("boundaries/cl.wkt"),
  }),
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

pub const COLOMBIA: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "CO",
    boundary_wkt: include_str!("boundaries/co.wkt"),
  }),
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

pub const ECUADOR: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "EC",
    boundary_wkt: include_str!("boundaries/ec.wkt"),
  }),
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:es", "name"],
    admin_levels: &[level::country, level::state, level::county, level::street],
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
    ("cdla.", "ciudadela"),
    ("urb.", "urbanización"),
    ("pza.", "plaza"),
    ("gral.", "general"),
    ("mcal.", "mariscal"),
    ("cnel.", "coronel"),
    ("tte.", "teniente"),
    ("dr.", "doctor"),
    ("sta.", "santa"),
    ("sto.", "santo"),
  ],
    ..DEFAULT.index_addresses
  },
};

pub const GUYANA: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "GY",
    boundary_wkt: include_str!("boundaries/gy.wkt"),
  }),
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:en", "name"],
    admin_levels: &[level::country, level::state, level::county, level::street],
  },
  house_numbers: DEFAULT.house_numbers,
  index_addresses: index_addresses_preset {
    abbreviations: &[
    ("rd.", "road"),
    ("st.", "street"),
    ("ave.", "avenue"),
    ("hwy.", "highway"),
    ("cres.", "crescent"),
    ("sq.", "square"),
    ("gdns.", "gardens"),
    ("pk.", "park"),
  ],
    ..DEFAULT.index_addresses
  },
};

pub const PARAGUAY: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "PY",
    boundary_wkt: include_str!("boundaries/py.wkt"),
  }),
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
    ("mcal.", "mariscal"),
    ("tte.", "teniente"),
    ("cnel.", "coronel"),
    ("gral.", "general"),
    ("pdte.", "presidente"),
    ("dr.", "doctor"),
    ("sta.", "santa"),
    ("sto.", "santo"),
  ],
    ..DEFAULT.index_addresses
  },
};

pub const PERU: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "PE",
    boundary_wkt: include_str!("boundaries/pe.wkt"),
  }),
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
    ("jr.", "jirón"),
    ("ca.", "calle"),
    ("av.", "avenida"),
    ("avda.", "avenida"),
    ("psje.", "pasaje"),
    ("pje.", "pasaje"),
    ("prol.", "prolongación"),
    ("urb.", "urbanización"),
    ("carr.", "carretera"),
    ("gral.", "general"),
    ("mcal.", "mariscal"),
    ("dr.", "doctor"),
    ("sta.", "santa"),
    ("sto.", "santo"),
  ],
    ..DEFAULT.index_addresses
  },
};

pub const SURINAME: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "SR",
    boundary_wkt: include_str!("boundaries/sr.wkt"),
  }),
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:nl", "name"],
    admin_levels: &[level::country, level::state, level::county, level::street],
  },
  house_numbers: DEFAULT.house_numbers,
  index_addresses: index_addresses_preset {
    abbreviations: &[
    ("str.", "straat"),
    ("ln.", "laan"),
    ("pl.", "plein"),
    ("st.", "sint"),
    ("dr.", "dokter"),
    ("mr.", "meester"),
    ("burg.", "burgemeester"),
    ("prof.", "professor"),
  ],
    ..DEFAULT.index_addresses
  },
};

pub const URUGUAY: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "UY",
    boundary_wkt: include_str!("boundaries/uy.wkt"),
  }),
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:es", "name"],
    admin_levels: &[level::country, level::city, level::street],
  },
  house_numbers: crate::house_number::house_number_policy {
    drop_values: &["s/n", "sn", "s/nº", "s/no", "s/n.", "s n"],
    ..DEFAULT.house_numbers
  },
  index_addresses: index_addresses_preset {
    abbreviations: &[
    ("av.", "avenida"),
    ("avda.", "avenida"),
    ("bvar.", "bulevar"),
    ("cno.", "camino"),
    ("rbla.", "rambla"),
    ("pje.", "pasaje"),
    ("gral.", "general"),
    ("pdte.", "presidente"),
    ("dr.", "doctor"),
    ("sta.", "santa"),
    ("sto.", "santo"),
  ],
    ..DEFAULT.index_addresses
  },
};

pub const VENEZUELA: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "VE",
    boundary_wkt: include_str!("boundaries/ve.wkt"),
  }),
  extract_osm_admin_levels: extract_osm_admin_levels_preset {
    name_priority: &["name:es", "name"],
    admin_levels: &[level::country, level::state, level::county, level::city, level::street],
  },
  house_numbers: crate::house_number::house_number_policy {
    drop_values: &["s/n", "sn", "s/nº", "s/no", "s/n.", "s n"],
    ..DEFAULT.house_numbers
  },
  index_addresses: index_addresses_preset {
    abbreviations: &[
    ("av.", "avenida"),
    ("avda.", "avenida"),
    ("urb.", "urbanización"),
    ("esq.", "esquina"),
    ("res.", "residencias"),
    ("ctra.", "carretera"),
    ("blvd.", "bulevar"),
    ("gral.", "general"),
    ("pdte.", "presidente"),
    ("dr.", "doctor"),
    ("sta.", "santa"),
    ("sto.", "santo"),
  ],
    ..DEFAULT.index_addresses
  },
};

pub const NETHERLANDS: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "NL",
    boundary_wkt: include_str!("boundaries/nl.wkt"),
  }),
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

pub const SWITZERLAND: preset = preset {
  country: Some(country {
    iso_3166_1_alpha_2: "CH",
    boundary_wkt: include_str!("boundaries/ch.wkt"),
  }),
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

pub const COUNTRIES: &[&preset] = &[
  &BRAZIL,
  &PORTUGAL,
  &ARGENTINA,
  &BOLIVIA,
  &CHILE,
  &COLOMBIA,
  &ECUADOR,
  &GUYANA,
  &PARAGUAY,
  &PERU,
  &SURINAME,
  &URUGUAY,
  &VENEZUELA,
  &NETHERLANDS,
  &SWITZERLAND,
];

pub fn for_country(iso: Option<&str>) -> preset {
  let Some(iso) = iso else {
    return DEFAULT;
  };
  COUNTRIES
    .iter()
    .find(|p| p.country.is_some_and(|c| c.iso_3166_1_alpha_2.eq_ignore_ascii_case(iso)))
    .map_or(DEFAULT, |p| **p)
}

pub fn name_tags() -> Vec<&'static str> {
  let mut tags: Vec<&'static str> = Vec::new();
  for preset in std::iter::once(&DEFAULT).chain(COUNTRIES.iter().copied()) {
    for &tag in preset.extract_osm_admin_levels.name_priority {
      if !tags.contains(&tag) {
        tags.push(tag);
      }
    }
  }
  tags
}
