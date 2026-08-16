// an openstreetmap relation — the `osm_data.osm_relations` table: an ordered list of members, each
// pointing at another element with a role. every administrative boundary above street level is one.

pub mod decoder;
pub mod entity;
pub mod payload;
pub mod repository;

// the member types are re-exported only where they are used: the fixture that builds relations
// reaches for `entity::` directly, so keeping them out of here avoids an export nothing consumes.
pub use entity::{osm_relation, osm_relation_row};
