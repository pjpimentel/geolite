pub mod boundaries;

mod default;
mod ar;
mod bo;
mod br;
mod ch;
mod cl;
mod co;
mod ec;
mod gy;
mod nl;
mod pe;
mod pt;
mod py;
mod sr;
mod uy;
mod ve;

pub use default::DEFAULT;

#[derive(Clone, Copy)]
pub struct country {
  pub iso_3166_1_alpha_2: &'static str,
  pub boundary_wkt: &'static str,
}

#[derive(Clone, Copy)]
pub struct preset {
  pub iso_3166_1_alpha_2: Option<&'static str>,
  pub extract_osm_admin_levels: extract_osm_admin_levels_preset,
  pub house_numbers: crate::house_number::house_number_policy,
  pub index_addresses: index_addresses_preset,
}

#[derive(Clone, Copy)]
pub struct extract_osm_admin_levels_preset {
  pub admin_levels: &'static [crate::admin_level::level],
  pub name_priority: &'static [&'static str],
}

#[derive(Clone, Copy)]
pub struct index_addresses_preset {
  pub abbreviations: &'static [(&'static str, &'static str)],
  pub boosts: crate::admin_level_hierarchy::tantivy_boosts,
}

pub const COUNTRY_PRESETS: &[&preset] = &[
  &ar::PRESET,
  &bo::PRESET,
  &br::PRESET,
  &ch::PRESET,
  &cl::PRESET,
  &co::PRESET,
  &ec::PRESET,
  &gy::PRESET,
  &nl::PRESET,
  &pe::PRESET,
  &pt::PRESET,
  &py::PRESET,
  &sr::PRESET,
  &uy::PRESET,
  &ve::PRESET,
];

pub fn for_country(iso: Option<&str>) -> preset {
  let Some(iso) = iso else {
    return DEFAULT;
  };
  COUNTRY_PRESETS
    .iter()
    .find(|p| p.iso_3166_1_alpha_2.is_some_and(|code| code.eq_ignore_ascii_case(iso)))
    .map_or(DEFAULT, |p| **p)
}

pub fn iso_key(iso: &str) -> Option<&'static str> {
  boundaries::ALL
    .iter()
    .map(|c| c.iso_3166_1_alpha_2)
    .find(|code| code.eq_ignore_ascii_case(iso))
}

pub fn name_tags() -> Vec<&'static str> {
  let mut tags: Vec<&'static str> = Vec::new();
  for preset in std::iter::once(&DEFAULT).chain(COUNTRY_PRESETS.iter().copied()) {
    for &tag in preset.extract_osm_admin_levels.name_priority {
      if !tags.contains(&tag) {
        tags.push(tag);
      }
    }
  }
  tags
}
