use rusqlite::Connection;

const SQL_ATTACH_SOURCE: &str = "ATTACH DATABASE ?1 AS merge_src";
const SQL_DETACH_SOURCE: &str = "DETACH DATABASE merge_src";

const SQL_MERGE_ADMIN_LEVELS: &str = "
  INSERT INTO admin_levels (
    id,
    relation_id,
    way_id,
    admin_level,
    wkb,
    name,
    country_iso_code,
    post_code
  )
  SELECT
    id,
    relation_id,
    way_id,
    admin_level,
    wkb,
    name,
    country_iso_code,
    post_code
  FROM merge_src.admin_levels
  WHERE TRUE
  ON CONFLICT (id) DO UPDATE SET
    name             = excluded.name,
    country_iso_code = excluded.country_iso_code,
    post_code        = excluded.post_code,
    wkb              = excluded.wkb
";

const SQL_MERGE_HOUSE_NUMBERS: &str = "
  INSERT OR IGNORE INTO house_numbers (
    node_id,
    admin_level_id,
    number,
    wkb,
    strategy
  )
  SELECT
    node_id,
    admin_level_id,
    number,
    wkb,
    strategy
  FROM merge_src.house_numbers
";

// attaches a source database read-only, merges its admin_levels (upsert by id) and house_numbers
// (insert-or-ignore by node_id) into the open base connection, then detaches. returns the number
// of (admin_levels, house_numbers) rows the merge touched.
pub fn merge_source(conn: &Connection, source_path: &str) -> (usize, usize) {
  let canonical = std::fs::canonicalize(source_path)
    .map(|p| p.to_string_lossy().into_owned())
    .unwrap_or_else(|_| source_path.to_string());
  let uri = format!("file:{canonical}?mode=ro");
  conn
    .execute(SQL_ATTACH_SOURCE, [&uri])
    .expect("failed to attach merge source");
  // admin_levels first: house_numbers.admin_level_id references admin_levels(id).
  let admins = conn
    .execute(SQL_MERGE_ADMIN_LEVELS, [])
    .expect("failed to merge admin_levels");
  let houses = conn
    .execute(SQL_MERGE_HOUSE_NUMBERS, [])
    .expect("failed to merge house_numbers");
  conn
    .execute(SQL_DETACH_SOURCE, [])
    .expect("failed to detach merge source");
  (admins, houses)
}

#[cfg(test)]
#[path = "merge.test.rs"]
mod tests;
