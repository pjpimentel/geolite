// the domain layer, organised as vertical slices: one folder per concept, each owning everything
// that concept needs — its model and methods, its policy, its services, and its own persistence.
//
// the technical modules that came before (`extract`, `index`, `query`, `optimize`, `database`) are
// being emptied into these folders and disappear as each concept lands. what stays outside is only
// what belongs to no concept in particular: the connection lifecycle, the cli and the http server.

pub mod admin_level;
pub mod house_number;
