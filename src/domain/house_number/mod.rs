// the house number context: what a door number is, how it is written, how it is recognised in a
// free-text query, how it is linked to a street and how it is resolved into a point.
//
// before this module the concept lived in four places with two different rules in two languages:
// normalisation in sql (the extraction query), the street link as a bare u8, token recognition in
// rust (the query path) and resolution alongside it. this module is the single definition.

pub mod link;
pub mod policy;
pub mod resolution;
pub mod token;
// the value object itself; re-exported below so call sites read
// `crate::domain::house_number::house_number` rather than repeating the segment.
pub mod value;

pub use link::{house_number_link, link_strategy};
pub use policy::house_number_policy;
pub use resolution::house_number_resolution;
pub use value::house_number;
