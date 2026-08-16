// which area contains which — the `admin_levels_hierarchy` table: for every admin level, the chain
// of areas that enclose it and the label you would read out loud, `"Rua Castro Alves, Embaré,
// Santos, São Paulo, Brasil"`.
//
// the row is one-to-one with an `admin_levels` row and cascades with it, but it is not a couple of
// columns of that table: it stores rendered content rather than recomputable acceleration, and it
// has readers of its own — the tantivy index, whose document is one per hierarchy row; the query
// path, which reads the label and the chain; and `optimize`, which refuses to run while it is empty.
//
// the chain points *upward*, denormalised onto the child, because every read starts from a leaf and
// works outward.

pub mod entity;
pub mod label;
pub mod repository;
pub mod resolver;

// only the read shape is re-exported: `hierarchy_row` is the write shape and never leaves the
// folder — the resolver builds it and the repository inserts it.
pub use entity::hierarchy_lookup_row;
