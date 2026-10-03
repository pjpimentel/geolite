use super::leaf::leaf;
use super::map;
use super::tree::folder;

pub fn print(folder: &folder) {
  let entries: Vec<(&str, String, String)> = folder
    .items
    .iter()
    .map(|item| {
      let inside = if item.is_folder {
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

pub fn print_leaf(leaf: &leaf) {
  let fields = leaf.fields();
  let name_w = fields
    .iter()
    .map(|field| field.name.len())
    .max()
    .unwrap_or(0);
  for field in fields {
    println!("{:<name_w$}  {}", field.name, field.shown());
  }
  println!();
  for line in map::text(&leaf.drawing) {
    println!("{line}");
  }
}
