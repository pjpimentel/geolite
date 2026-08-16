// a source `.osm.pbf` file — the `osm_pbf_files` table: one row per file, from the moment it is
// found in the geofabrik catalogue to the counts left behind after extraction.
//
// the row is a ledger written in column groups — geofabrik metadata, download state, osm header,
// counts — and read back only by `file_path`, `geofabrik_url` and `id`. nothing ever reads it
// whole, which is why this slice has no `entity`: the ddl in `repository` is the shape.
//
// the file's byte layout is a second, subordinate table — `blob_index`, the same arrangement
// `admin_level` uses for its rtree. not to be confused with `crate::pbf`, which is the wire format
// every osm element is decoded from.

pub mod blob_index;
pub mod blob_scanner;
pub mod catalog;
pub mod download;
pub mod header;
pub mod http_client;
pub mod repository;

pub use blob_index::{chunk_type, osm_pbf_blob_chunk};

#[cfg(test)]
#[path = "http_stubs.test.rs"]
pub(crate) mod http_stubs;
