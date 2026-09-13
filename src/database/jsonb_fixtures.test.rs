// sqlite itself is the oracle of the format: a malformed jsonb makes JSON() fail or answer
// something other than the value written
pub(crate) fn to_json(payload: &[u8]) -> serde_json::Value {
  let conn = rusqlite::Connection::open_in_memory().expect("failed to open sqlite");
  let text: String = conn
    .query_row("SELECT JSON(?1)", rusqlite::params![payload], |row| row.get(0))
    .expect("sqlite could not read the jsonb produced");
  serde_json::from_str(&text).expect("sqlite answered invalid json")
}

pub(crate) fn tags(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
  pairs
    .iter()
    .map(|&(k, v)| (k.to_string(), v.to_string()))
    .collect()
}
