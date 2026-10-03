use serde::Serialize;
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub enum house_number_scenario {
  from_osm_data,
  presumed_from_multiple_references_from_street,
  presumed_from_one_ref_from_street,
  presumed_from_constants,
}
