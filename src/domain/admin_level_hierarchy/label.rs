pub fn root(name: &str, own_post_code: Option<&str>) -> String {
  with_post_code(name, own_post_code)
}

pub fn nested(name: &str, parent_label: &str, own_post_code: Option<&str>) -> String {
  with_post_code(&format!("{name}, {parent_label}"), own_post_code)
}

fn with_post_code(base: &str, own_post_code: Option<&str>) -> String {
  match own_post_code.map(str::trim).filter(|s| !s.is_empty()) {
    Some(pc) => format!("{base}, {pc}"),
    None => base.to_string(),
  }
}

#[cfg(test)]
#[path = "label.test.rs"]
mod tests;
