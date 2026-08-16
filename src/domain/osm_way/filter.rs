// which ways a level's extraction wants, expressed as meaning rather than as sql.
//
// osm has no "this is a street" tag; a street is a way that carries certain tags and not others.
// each variant names one such condition, and the repository is what turns it into a predicate.
// keeping the meaning here is what lets a preset say `exclude_building` without knowing that a
// column called `payload` holds json.
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
