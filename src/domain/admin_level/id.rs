// stable identity of an admin level, derived from the osm element that produced it.
//
// ways and relations number their elements in separate namespaces, so the same numeric osm id may
// name both a way and a relation. the low bit carries the element kind and the remaining bits carry
// the osm id, which keeps a single integer key disjoint across both namespaces.
//
// the packing is part of the on-disk format (`admin_levels.id`) and must not change.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct admin_level_id(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum osm_element_kind {
  way,
  relation,
}

impl admin_level_id {
  pub fn from_way(osm_id: u64) -> Self {
    Self(osm_id << 1)
  }

  pub fn from_relation(osm_id: u64) -> Self {
    Self((osm_id << 1) | 1)
  }

  // rebuilds the identity from a value already stored in the database.
  pub fn from_raw(raw: u64) -> Self {
    Self(raw)
  }

  pub fn raw(self) -> u64 {
    self.0
  }

  #[allow(dead_code)]
  pub fn kind(self) -> osm_element_kind {
    if self.0 & 1 == 1 {
      osm_element_kind::relation
    } else {
      osm_element_kind::way
    }
  }

  #[allow(dead_code)]
  pub fn osm_id(self) -> u64 {
    self.0 >> 1
  }
}

#[cfg(test)]
#[path = "id.test.rs"]
mod tests;
