// the openstreetmap tag vocabulary: the keys geolite interprets, the values it treats as closed
// sets, and how a tag is read back out of a stored payload.
//
// two boundaries define this folder, and both matter more than what is inside it:
//
// - it owns **the key and the shape of its value** — the osm literal, the json path, the closed
//   value set where there is one, the normalisation. it does not own what a *combination* of tags
//   means: "a way with a highway tag and no building tag is a street" is `osm_way::way_filter`,
//   and which tags carry a house number is `house_number::policy`. blurring that line would undo
//   the split those slices were built on.
// - it names the **interpreted** vocabulary, not the stored one. `pbf::tag_policy` keeps whatever
//   the file carries — an absent include list means "every tag" — and stays stringly-typed on
//   purpose, because the pipeline stores keys nobody here has heard of.
//
// unlike every other folder under `domain`, this one is not a table. it is shared vocabulary, the
// way `admin_level::level` is a scale rather than a row, and it is here rather than beside `pbf`
// because a tag carries meaning: `pbf` is format, this is what the format is saying.

pub mod key;
pub mod select;
pub mod value;

pub use key::osm_tag;
