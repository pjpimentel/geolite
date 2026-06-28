const SQL_PAGE_SIZE: &str = "PRAGMA page_size";
const SQL_PAGE_COUNT: &str = "PRAGMA page_count";
const SQL_OPTIMIZE: &str = "
  ANALYZE;
  PRAGMA optimize;
  PRAGMA wal_checkpoint(TRUNCATE);
  VACUUM;
";

pub fn run(conn: &rusqlite::Connection) -> (u64, u64) {
  let page_size: u64 = conn
    .query_row(SQL_PAGE_SIZE, [], |r| r.get(0))
    .unwrap_or(4096);
  let pages_before: u64 = conn
    .query_row(SQL_PAGE_COUNT, [], |r| r.get(0))
    .unwrap_or(0);
  conn
    .execute_batch(SQL_OPTIMIZE)
    .expect("failed to optimize sqlite file");
  let pages_after: u64 = conn
    .query_row(SQL_PAGE_COUNT, [], |r| r.get(0))
    .unwrap_or(0);
  (page_size * pages_before, page_size * pages_after)
}
