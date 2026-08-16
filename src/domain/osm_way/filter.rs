#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum way_filter {
  include_place_neighbourhood,
  include_place_suburb,
  include_highway_residential,
  include_highway_primary,
  include_highway_secondary,
  include_highway_tertiary,
  include_highway_unclassified,
  include_highway_living_street,
  exclude_place_neighbourhood,
  exclude_place_suburb,
  exclude_leisure_park,
  exclude_building,
  exclude_waterway,
}
