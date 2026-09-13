use super::insert_in_chunks;

const SQL_INSERT_HEAD: &str = "
  INSERT OR IGNORE INTO rows (
    id,
    label
  ) VALUES
";

fn scratch_table() -> rusqlite::Connection {
  let conn = rusqlite::Connection::open_in_memory().expect("failed to open sqlite");
  conn
    .execute_batch("CREATE TABLE rows (id INTEGER PRIMARY KEY, label TEXT NOT NULL);")
    .expect("failed to create the scratch table");
  conn
}

fn count(conn: &rusqlite::Connection) -> i64 {
  conn
    .query_row("SELECT COUNT(*) FROM rows", [], |row| row.get(0))
    .expect("failed to count")
}

fn label_of(conn: &rusqlite::Connection, id: i64) -> String {
  conn
    .query_row("SELECT label FROM rows WHERE id = ?1", [id], |row| row.get(0))
    .expect("failed to read the label")
}

#[test]
fn _00_keeps_every_row_across_the_chunk_boundary() {
  let conn = scratch_table();
  let ids: Vec<i64> = (1..=10_001).collect();
  let labels: Vec<String> = ids.iter().map(|id| format!("row {id}")).collect();
  let params: Vec<[&dyn rusqlite::ToSql; 2]> = ids
    .iter()
    .zip(&labels)
    .map(|(id, label)| [id as &dyn rusqlite::ToSql, label])
    .collect();

  insert_in_chunks(&conn, SQL_INSERT_HEAD, &params);

  assert_eq!(count(&conn), 10_001);
  assert_eq!(label_of(&conn, 1), "row 1");
  assert_eq!(label_of(&conn, 10_001), "row 10001");
}

#[test]
fn _01_leaves_the_first_row_in_place_when_the_head_says_or_ignore() {
  let conn = scratch_table();

  insert_in_chunks(&conn, SQL_INSERT_HEAD, &[[&1i64 as &dyn rusqlite::ToSql, &"first"]]);
  insert_in_chunks(&conn, SQL_INSERT_HEAD, &[[&1i64 as &dyn rusqlite::ToSql, &"second"]]);

  assert_eq!(count(&conn), 1);
  assert_eq!(label_of(&conn, 1), "first");
}

#[test]
fn _02_does_nothing_without_rows() {
  let conn = scratch_table();
  let params: Vec<[&dyn rusqlite::ToSql; 2]> = Vec::new();

  insert_in_chunks(&conn, SQL_INSERT_HEAD, &params);

  assert_eq!(count(&conn), 0);
}
