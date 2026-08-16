use rusqlite::Connection;

// the bounding box of every admin level, in a sqlite rtree virtual table. it is a derived index:
// dropped and rebuilt wholesale from the geometries, never updated in place.
const SQL_CREATE_RTREE: &str = "
  CREATE VIRTUAL TABLE IF NOT EXISTS admin_levels_rtree
  USING rtree(id, min_lon, max_lon, min_lat, max_lat);
";

const SQL_DROP_RTREE: &str = "DROP TABLE IF EXISTS admin_levels_rtree;";

const SQL_INSERT_RTREE: &str = "
  INSERT INTO admin_levels_rtree (
    id,
    min_lon,
    max_lon,
    min_lat,
    max_lat
  ) VALUES (
    ?1,
    ?2,
    ?3,
    ?4,
    ?5
  );
";

pub struct rtree_row {
  pub id: i64,
  pub min_lon: f64,
  pub max_lon: f64,
  pub min_lat: f64,
  pub max_lat: f64,
}

pub(crate) fn create(conn: &Connection) {
  conn
    .execute_batch(SQL_CREATE_RTREE)
    .expect("failed to create admin_levels_rtree");
}

pub(crate) fn drop_table(conn: &Connection) {
  conn
    .execute_batch(SQL_DROP_RTREE)
    .expect("failed to drop admin_levels_rtree");
}

pub fn recreate(conn: &Connection) {
  drop_table(conn);
  create(conn);
}

pub fn batch_insert(conn: &Connection, rows: &[rtree_row]) {
  if rows.is_empty() {
    return;
  }
  let tx = conn
    .unchecked_transaction()
    .expect("failed to begin transaction");
  {
    let mut stmt = tx
      .prepare(SQL_INSERT_RTREE)
      .expect("failed to prepare rtree insert");
    rows
      .iter()
      .try_for_each(|row| {
        stmt
          .execute(rusqlite::params![
            row.id,
            row.min_lon,
            row.max_lon,
            row.min_lat,
            row.max_lat
          ])
          .map(|_| ())
      })
      .expect("failed to insert rtree row");
  }
  tx.commit().expect("failed to commit rtree batch");
}

