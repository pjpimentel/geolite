// a door number placed on a street — the `house_numbers` table: what a number is, how it is
// written, how it is recognised inside a free-text query, how it is linked to a street and how it
// is resolved into a point.
//
// before this module the concept lived in four places with two different rules in two languages:
// normalisation in sql (the extraction query), the street link as a bare u8, token recognition in
// rust (the query path) and resolution alongside it. this module is the single definition.

pub mod entity;
pub mod policy;
pub mod repository;
pub mod resolution;
pub mod strategy;
pub mod token;
pub mod value;

pub use entity::house_number_link;
pub use policy::house_number_policy;
pub use resolution::house_number_resolution;
pub use strategy::link_strategy;
pub use value::house_number;
