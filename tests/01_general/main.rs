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

#[path = "admin_level_hierarchy.test.rs"]
mod admin_level_hierarchy;

#[path = "house_number.test.rs"]
mod house_number;

#[path = "isolated_admin_levels.test.rs"]
mod isolated_admin_levels;

#[path = "build.test.rs"]
mod build;

#[path = "street_merge.test.rs"]
mod street_merge;

#[path = "tui.test.rs"]
mod tui;

#[path = "etag.test.rs"]
mod etag;

#[path = "south_america.test.rs"]
mod south_america;
