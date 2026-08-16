// an openstreetmap node — the `osm_data.osm_nodes` table: a point with tags, the rawest thing the
// pipeline stores. it is read from the pbf wire format, written as a jsonb payload, and later
// scanned for addresses and for the coordinates of ways and relations.
//
// the payload is encoded up front, in parallel, which is why the storage row is part of the model
// rather than hidden in the repository — see `entity`.

pub mod decoder;
pub mod entity;
pub mod payload;
pub mod repository;

pub use entity::{osm_node, osm_node_row};
