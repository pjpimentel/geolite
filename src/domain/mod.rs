pub mod admin_level;
pub mod admin_level_hierarchy;
pub mod osm_node;
pub mod osm_pbf_file;
pub mod osm_relation;
pub mod osm_tag;
pub mod osm_way;

pub(crate) trait table {
  const CREATE: &'static str;
  const INDEXES: &'static str;

  fn create_table(conn: &rusqlite::Connection) {
    conn
      .execute_batch(Self::CREATE)
      .expect("failed to create table");
  }

  fn create_indexes(conn: &rusqlite::Connection) {
    conn
      .execute_batch(Self::INDEXES)
      .expect("failed to create indexes");
  }
}
