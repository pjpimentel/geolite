// the domain layer: the model itself, free of i/o.
//
// nothing under `domain` may import rusqlite, geozero, tantivy, ureq, tiny_http, clap, prost or
// serde. the only external crate allowed here is `geo`, which is a geometry model rather than an
// i/o concern. everything else belongs to the adapters that surround this layer.

pub mod house_number;
pub mod kernel;
