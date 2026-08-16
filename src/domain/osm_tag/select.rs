use super::key::{json_path_of, osm_tag};

fn extract(payload_expr: &str, tag: osm_tag) -> String {
  format!("JSON_EXTRACT({payload_expr}, '{}')", tag.json_path())
}

fn quote(value: &str) -> String {
  format!("'{}'", value.replace('\'', "''"))
}

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
