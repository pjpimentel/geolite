// a named administrative area — the `admin_levels` table: a country, a state, a city, a
// neighborhood, a street. everything the concept needs lives here, from the scale it sits on to
// the sql that reads and writes it.

pub mod entity;
pub mod geometry;
pub mod id;
pub mod repository;
pub mod scale;
pub mod spatial_index;

pub use entity::admin_level;
pub use id::admin_level_id;
pub use scale::level;
