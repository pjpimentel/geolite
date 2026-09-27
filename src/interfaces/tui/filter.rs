use super::tree::folder;
use crate::admin_level_hierarchy::search_index::tokenize;

pub struct filter {
  pub text: String,
  pub shown: Vec<usize>,
  names: Vec<String>,
}

impl filter {
  // the names are folded once per folder, so a key typed only compares: folding them again at
  // every key builds one analyzer per item of the folder
  pub fn over(folder: &folder) -> filter {
    let names: Vec<String> = folder
      .items
      .iter()
      .map(|item| folded(&item.entry.name()))
      .collect();
    filter {
      text: String::new(),
      shown: (0..names.len()).collect(),
      names,
    }
  }

  pub fn push(&mut self, typed: char) {
    self.text.push(typed);
    self.apply();
  }

  pub fn pop(&mut self) {
    self.text.pop();
    self.apply();
  }

  pub fn clear(&mut self) {
    self.text.clear();
    self.apply();
  }

  fn apply(&mut self) {
    let wanted = folded(&self.text);
    self.shown = self
      .names
      .iter()
      .enumerate()
      .filter(|(_, name)| name.contains(&wanted))
      .map(|(index, _)| index)
      .collect();
  }
}

fn folded(text: &str) -> String {
  tokenize(text).join(" ")
}
