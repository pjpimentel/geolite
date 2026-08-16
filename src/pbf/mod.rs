// the osm pbf wire format: the protobuf messages, how a blob is decompressed and which tags
// survive extraction.
//
// it belongs to no table and it is not a pipeline stage, so it sits beside `database` rather than
// inside either. the element domains read it to decode themselves; the extraction pipeline reads it
// to walk the file. not to be confused with `osm_pbf_file`, which is the catalogue and the
// download.

pub mod blob;
pub mod message;
pub mod tag_policy;
