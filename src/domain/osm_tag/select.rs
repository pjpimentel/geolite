use super::key::json_path_of;

pub fn coalesce_of(payload_expr: &str, keys: &[&str]) -> String {
  let parts: Vec<String> = keys
    .iter()
    .map(|key| format!("JSON_EXTRACT({payload_expr}, '{}')", json_path_of(key)))
    .collect();
  match parts.len() {
    0 => format!("JSON_EXTRACT({payload_expr}, '{}')", json_path_of("name")),
    1 => parts.into_iter().next().unwrap(),
    _ => format!("COALESCE({})", parts.join(", ")),
  }
}

#[cfg(test)]
#[path = "select.test.rs"]
mod tests;
