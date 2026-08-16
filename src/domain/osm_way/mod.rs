// an openstreetmap way — the `osm_data.osm_ways` table: an ordered list of node references with
// tags. streets and neighbourhood outlines are both ways; which tags make one or the other is a
// per-level decision, expressed by `filter`.

pub mod decoder;
pub mod entity;
pub mod filter;
pub mod payload;
pub mod repository;

pub use entity::{osm_way, osm_way_row};
pub use filter::way_filter;
