// the names from the area outward, then the post codes from the root inward: what the stored
// label used to read, composed at query time from the path
pub fn render<'a>(
  own: (&'a str, Option<&'a str>),
  ancestors: impl Iterator<Item = (&'a str, Option<&'a str>)>,
) -> String {
  let mut names: Vec<&str> = vec![own.0];
  let mut post_codes: Vec<Option<&str>> = vec![own.1];
  for (name, post_code) in ancestors {
    names.push(name);
    post_codes.push(post_code);
  }
  let mut label = names.join(", ");
  for post_code in post_codes.iter().rev() {
    if let Some(pc) = post_code.map(str::trim).filter(|s| !s.is_empty()) {
      label.push_str(", ");
      label.push_str(pc);
    }
  }
  label
}
