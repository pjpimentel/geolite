use super::tree::folder;

pub fn print(folder: &folder) {
  let entries: Vec<(&str, String, String)> = folder
    .items
    .iter()
    .map(|item| {
      let inside = if item.entry.is_folder() {
        item.inside.to_string()
      } else {
        String::new()
      };
      (item.entry.level().name(), inside, item.entry.name())
    })
    .collect();
  if entries.is_empty() {
    println!("no places under {}", folder.breadcrumb());
    return;
  }
  let level_w = entries
    .iter()
    .map(|(level, _, _)| level.len())
    .max()
    .unwrap_or(0)
    .max(5);
  let inside_w = entries
    .iter()
    .map(|(_, inside, _)| inside.len())
    .max()
    .unwrap_or(0)
    .max(6);
  let name_w = entries
    .iter()
    .map(|(_, _, name)| name.chars().count())
    .max()
    .unwrap_or(0)
    .max(4);
  println!("{:<level_w$}  {:>inside_w$}  {:<name_w$}", "level", "inside", "name");
  println!("{:-<level_w$}  {:-<inside_w$}  {:-<name_w$}", "", "", "");
  for (level, inside, name) in entries {
    println!("{level:<level_w$}  {inside:>inside_w$}  {name:<name_w$}");
  }
}
