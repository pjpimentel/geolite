pub mod admin_levels;
pub mod house_numbers;
pub mod osm_data;

#[cfg(test)]
#[path = "pbf_fixtures.test.rs"]
pub(crate) mod pbf_fixtures;

#[cfg(test)]
#[path = "extract.test.rs"]
mod tests;