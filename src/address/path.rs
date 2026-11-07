use rusqlite::Connection;

use super::entity::{leaf, match_sources, query_match};
use super::house_number;
use super::text::resting_point;

pub(super) fn run(
  conn: &Connection,
  iso: &str,
  id: i64,
  path: &[i64],
  number: Option<&str>,
) -> Option<query_match> {
  let mut sources = match_sources::load(conn, iso, &[id]);
  if !sources.paths_of(id).iter().any(|known| known == path) {
    return None;
  }
  sources.load_leaf_boxes(conn);
  let record = crate::admin_level::repository::load_full_by_ids(conn, &[id]).pop()?;
  let geometry = record.wkb.as_ref()?.geometry();
  let resting = resting_point(&record, geometry, &sources, path)?;
  let placed = match number {
    Some(number) => {
      let policy = sources.house_number_policy_of(id);
      Some(house_number::on_street(conn, number, &record, &policy)?)
    }
    None => None,
  };
  let point = placed.as_ref().map_or(resting, |placed| placed.point);
  let reported = placed.as_ref().map(house_number::reported);
  let ancestors = sources.ancestors_of(path.iter());
  let leaf = leaf {
    id: record.id,
    level: record.admin_level,
    name: &record.name,
    relation_id: record.relation_id,
    way_id: record.way_id,
  };
  Some(sources.match_at(&leaf, &ancestors, path, point, reported.as_ref(), None))
}
