pub mod entity;
pub mod extract;
mod linker;
pub mod policy;
pub mod repository;
pub mod resolution;
pub mod strategy;
pub mod token;
pub mod value;

#[cfg(test)]
#[path = "fixtures.test.rs"]
pub(crate) mod fixtures;

pub use entity::house_number_link;
pub use policy::house_number_policy;
pub use repository::house_numbers;
pub use resolution::house_number_resolution;
pub use value::house_number;
