// which osm tags survive extraction.
//
// both lists are optional and independent: an absent include list means "every tag", an absent
// ignore list means "drop nothing". a tag has to pass both to be kept.
#[derive(Default)]
pub struct tag_policy {
  pub include: Option<Vec<String>>,
  pub ignore: Option<Vec<String>>,
}

impl tag_policy {
  pub fn passes(&self, key: &str) -> bool {
    self.include.as_ref().is_none_or(|l| l.iter().any(|i| i == key))
      && self.ignore.as_ref().is_none_or(|l| !l.iter().any(|i| i == key))
  }

  // the key/value index pairs of an element, resolved against the block's string table and
  // filtered. returns borrowed slices — the caller owns the decision to allocate.
  pub fn filter<'a>(
    &self,
    strings: &[&'a str],
    keys: &[u32],
    vals: &[u32],
  ) -> Vec<(&'a str, &'a str)> {
    keys
      .iter()
      .zip(vals.iter())
      .filter_map(|(&k_idx, &v_idx)| {
        let k = strings.get(k_idx as usize)?;
        let v = strings.get(v_idx as usize)?;
        if self.passes(k) { Some((*k, *v)) } else { None }
      })
      .collect()
  }
}

#[cfg(test)]
#[path = "tag_policy.test.rs"]
mod tests;
