use rusqlite::Connection;

use super::entity::{leaf, match_sources, query_match};
use super::text::resting_point;

pub(super) fn run(conn: &Connection, id: i64, path: &[i64]) -> Option<query_match> {
  let mut sources = match_sources::load(conn, &[id], false);
  sources.load_leaf_boxes(conn);
  let record = crate::admin_level::repository::load_full_by_ids(conn, &[id]).pop()?;
  let point = resting_point(&record, record.wkb.as_ref()?.geometry(), &sources, path)?;
  let ancestors = sources.ancestors_of(path.iter());
  let leaf = leaf {
    id: record.id,
    level: record.admin_level,
    name: &record.name,
    relation_id: record.relation_id,
    way_id: record.way_id,
  };
  Some(sources.match_at(&leaf, &ancestors, path, point, None, None))
}
