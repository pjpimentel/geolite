#![allow(nonstandard_style)]

#[path = "../00_common/mod.rs"]
mod common;

#[path = "general.test.rs"]
mod general;

#[path = "osm_pbf_file.test.rs"]
mod osm_pbf_file;

#[path = "extract.test.rs"]
mod extract;

#[path = "admin_level.test.rs"]
mod admin_level;

#[path = "house_number.test.rs"]
mod house_number;
