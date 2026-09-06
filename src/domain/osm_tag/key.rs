pub fn json_path_of(key: &str) -> String {
  format!("$.tags.\"{key}\"")
}

pub fn is_valid_key(key: &str) -> bool {
  !key.is_empty()
    && key
      .chars()
      .all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '-' || c == '_')
}

#[cfg(test)]
#[path = "key.test.rs"]
mod tests;
