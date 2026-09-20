pub mod entity;
pub mod paths;
pub mod repository;
pub mod resolver;
pub mod search_index;
pub mod street_merge;

#[cfg(test)]
#[path = "fixtures.test.rs"]
pub(crate) mod fixtures;

pub use repository::admin_levels_hierarchy;
pub use search_index::{tantivy_boosts, tantivy_index};
