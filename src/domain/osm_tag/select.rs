// sql expressions that read a tag out of an element's `payload` column.
//
// every path this crate builds goes through here, so the quoting rule lives in one place. what a
// combination of tags *means* is not decided here — `osm_way::way_filter` says which ways a level
// wants, and this module only knows how to write each condition down.
//
// `payload_expr` is the qualified column expression, e.g. `osm_data.osm_ways.payload`, or just
// `payload` where the query has a single table in scope.

use super::key::{json_path_of, osm_tag};

fn extract(payload_expr: &str, tag: osm_tag) -> String {
  format!("JSON_EXTRACT({payload_expr}, '{}')", tag.json_path())
}

// sql string literals are single-quoted; a value carrying one would end the literal early. every
// caller passes a compile-time constant today, and this keeps that from being load-bearing.
fn quote(value: &str) -> String {
  format!("'{}'", value.replace('\'', "''"))
}

// the first key present wins, over keys given as free text.
pub fn coalesce_of(payload_expr: &str, keys: &[&str]) -> String {
  let parts: Vec<String> = keys
    .iter()
    .map(|key| format!("JSON_EXTRACT({payload_expr}, '{}')", json_path_of(key)))
    .collect();
  match parts.len() {
    0 => extract(payload_expr, osm_tag::name),
    1 => parts.into_iter().next().unwrap(),
    _ => format!("COALESCE({})", parts.join(", ")),
  }
}

// the first key present wins, trimmed and upper-cased, with blank read as absent. this is how a
// country code and a post code are lifted off an element into an admin level's columns.
pub fn normalized_coalesce(payload_expr: &str, tags: &[osm_tag]) -> String {
  let parts: Vec<String> = tags
    .iter()
    .map(|&tag| extract(payload_expr, tag))
    .collect();
  format!(
    "NULLIF(UPPER(TRIM(COALESCE(\n           {}\n         ))), '')",
    parts.join(",\n           ")
  )
}

pub fn equals(payload_expr: &str, tag: osm_tag, value: &str) -> String {
  format!("{} = {}", extract(payload_expr, tag), quote(value))
}

pub fn not_in(payload_expr: &str, tag: osm_tag, values: &[&str]) -> String {
  let list: Vec<String> = values.iter().map(|v| quote(v)).collect();
  format!(
    "COALESCE({}, '') NOT IN ({})",
    extract(payload_expr, tag),
    list.join(", ")
  )
}

pub fn is_null(payload_expr: &str, tag: osm_tag) -> String {
  format!("{} IS NULL", extract(payload_expr, tag))
}

pub fn is_not_null(payload_expr: &str, tag: osm_tag) -> String {
  format!("{} IS NOT NULL", extract(payload_expr, tag))
}

#[cfg(test)]
#[path = "select.test.rs"]
mod tests;
