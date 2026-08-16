// the `user_friendly_name` rule: an area's own name, then the label its parent already carries,
// then its own post code if it has one.
//
// composing from the parent's finished label rather than from the chain of ids is what makes the
// rule a single step — every ancestor's own post code is already inside the string it hands down.
// this is the text the whole search index is built on.

// an area with no enclosing parent: only its own name and post code.
pub fn root(name: &str, own_post_code: Option<&str>) -> String {
  with_post_code(name, own_post_code)
}

// an area inside another: its name, the parent's finished label, then its own post code.
pub fn nested(name: &str, parent_label: &str, own_post_code: Option<&str>) -> String {
  with_post_code(&format!("{name}, {parent_label}"), own_post_code)
}

fn with_post_code(base: &str, own_post_code: Option<&str>) -> String {
  match own_post_code.map(str::trim).filter(|s| !s.is_empty()) {
    Some(pc) => format!("{base}, {pc}"),
    None => base.to_string(),
  }
}
