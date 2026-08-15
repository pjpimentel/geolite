use super::policy::house_number_policy;

// a door number, in the two forms the system needs it in:
//
// - `stored_form` is what gets persisted or shown. it is produced faithfully by whichever side
//   built the value: the ingestion side canonicalises the osm tag, the query side keeps the token
//   as the user wrote it (minus the punctuation that introduces it).
// - `comparison_key` is the aggressively canonical form, and it is the ONLY thing equality looks
//   at. two numbers are the same number when their keys match.
//
// the split is what lets the colombian nomenclature be fixed without a rebuild: databases already
// on disk keep the exact bytes they were written with, because the key is computed in memory, at
// comparison time, on both sides.
#[derive(Clone, Debug)]
pub struct house_number {
  stored_form: String,
  // `None` means the key is the stored form itself, which is the case for a plain number — the
  // overwhelming majority of rows. extraction materialises every candidate in memory, so skipping
  // the second allocation there is worth the branch here.
  key: Option<String>,
  shape: house_number_shape,
}

// the written form of a number. `compound` and a numeric range are syntactically identical
// (`82-52` vs `12-14`), which is exactly why recognition is policy-driven: the string cannot tell
// them apart, only the region can. anything the parser does not recognise is `free` — it is still
// stored (osm holds values like "Lote 5"), it just never resolves a query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum house_number_shape {
  simple,
  suffixed,
  compound,
  free,
}

impl PartialEq for house_number {
  fn eq(&self, other: &Self) -> bool {
    self.comparison_key() == other.comparison_key()
  }
}

impl house_number {
  // ingestion side: turns a raw osm tag value into the value to store.
  //
  // this reproduces, character for character, what the extraction query used to do in sql, so
  // moving the rule into rust does not rewrite a single existing row: trim spaces, drop empties
  // and the region's non-values, then canonicalise a trailing letter suffix ("12 a", "12-a" and
  // "12a" all become "12A"). everything else is stored untouched.
  pub fn normalize(raw: &str, policy: &house_number_policy) -> Option<Self> {
    // sqlite's TRIM removes spaces only, not every unicode whitespace — matched on purpose.
    let trimmed = raw.trim_matches(' ');
    if trimmed.is_empty() {
      return None;
    }
    // the drop list is checked against the trimmed value, before canonicalisation, as the sql
    // WHERE clause did (it referenced the cte column, not the projected one).
    if policy
      .drop_values
      .iter()
      .any(|value| value.eq_ignore_ascii_case(trimmed))
    {
      return None;
    }
    Some(Self::from_text(&canonical_suffix(trimmed)))
  }

  // query side: decides whether a token the user typed is a door number at all.
  //
  // strict by design — a token that is not clearly a number must fall through, so that postcodes
  // and street-name numbers are never mistaken for one.
  pub fn recognize(token: &str, policy: &house_number_policy) -> Option<Self> {
    let token = token.trim_matches(' ');
    let core = token.strip_suffix(',').unwrap_or(token);
    let core = if policy.allow_hash_prefix {
      core.strip_prefix('#').unwrap_or(core).trim_start_matches(' ')
    } else {
      core
    };
    if core.is_empty() {
      return None;
    }
    let shape = classify(core);
    if !policy.shapes.contains(&shape) {
      return None;
    }
    if !digits_within(core, shape, policy.max_digits) {
      return None;
    }
    Some(Self::from_text(core))
  }

  // read-back side: a value already written to storage is canonical by construction, so it needs
  // no policy — reading is not ingesting, and re-applying the drop list here would discard rows
  // that are legitimately on disk.
  pub fn from_stored(text: &str) -> Self {
    Self::from_text(text)
  }

  fn from_text(text: &str) -> Self {
    let key = comparison_key_of(text);
    Self {
      key: (key != text).then_some(key),
      shape: classify(text),
      stored_form: text.to_string(),
    }
  }

  pub fn stored_form(&self) -> &str {
    &self.stored_form
  }

  pub fn comparison_key(&self) -> &str {
    self.key.as_deref().unwrap_or(&self.stored_form)
  }

  pub fn shape(&self) -> house_number_shape {
    self.shape
  }

  // the leading run of digits, used to order numbers along a street for interpolation.
  // "123a" -> 123, "s/n" -> None.
  pub fn leading_value(&self) -> Option<u32> {
    let digits: String = self
      .stored_form
      .chars()
      .take_while(|c| c.is_ascii_digit())
      .collect();
    digits.parse().ok()
  }
}

// canonicalises a trailing letter suffix, mirroring the CASE that used to live in the extraction
// query. the two arms are tried in the same order sqlite tried them.
fn canonical_suffix(text: &str) -> String {
  let chars: Vec<char> = text.chars().collect();
  let length = chars.len();
  let last = chars[length - 1];
  if !last.is_ascii_alphabetic() {
    return text.to_string();
  }
  // "12 a" / "12-a": a separator sits between the digits and the letter.
  if length >= 3 && (chars[length - 2] == ' ' || chars[length - 2] == '-') {
    let prefix = &chars[..length - 2];
    if prefix.iter().all(char::is_ascii_digit) {
      return suffixed(prefix, last);
    }
  }
  // "12a": the letter is attached to the digits.
  if length >= 2 {
    let prefix = &chars[..length - 1];
    if prefix.iter().all(char::is_ascii_digit) {
      return suffixed(prefix, last);
    }
  }
  text.to_string()
}

fn suffixed(digits: &[char], letter: char) -> String {
  let mut out: String = digits.iter().collect();
  out.push(letter.to_ascii_uppercase());
  out
}

// the policy-independent canonical form used for equality. it must not consult a policy: the
// stored side and the query side compute it separately and have to agree.
fn comparison_key_of(text: &str) -> String {
  let stripped = text
    .strip_prefix('#')
    .unwrap_or(text)
    .trim_matches(' ')
    .to_ascii_uppercase();
  match split_compound(&stripped) {
    Some((first, second)) => format!("{first}-{second}"),
    None => stripped,
  }
}

fn classify(text: &str) -> house_number_shape {
  let chars: Vec<char> = text.chars().collect();
  if chars.is_empty() {
    return house_number_shape::free;
  }
  if chars.iter().all(char::is_ascii_digit) {
    return house_number_shape::simple;
  }
  let digits = chars.iter().take_while(|c| c.is_ascii_digit()).count();
  if digits > 0 && chars.len() - digits == 1 && chars[digits].is_ascii_alphabetic() {
    return house_number_shape::suffixed;
  }
  if split_compound(&text.to_ascii_uppercase()).is_some() {
    return house_number_shape::compound;
  }
  house_number_shape::free
}

// splits `<digits><letter?>` `-` or the letter itself `<digits><letter?>` into its two parts:
//   "82-52" -> ("82", "52")     "25B-48" -> ("25B", "48")     "16I56" -> ("16I", "56")
// a leading zero on the first part rejects the match, which keeps a hyphenated postcode
// ("01310-100") from being read as a compound number.
fn split_compound(text: &str) -> Option<(String, String)> {
  let chars: Vec<char> = text.chars().collect();
  let mut at = 0;

  let first_digits = run_of_digits(&chars, &mut at);
  if first_digits.is_empty() || (first_digits.len() > 1 && first_digits.starts_with('0')) {
    return None;
  }
  let first_letter = optional_letter(&chars, &mut at);
  let separated_by_hyphen = chars.get(at) == Some(&'-');
  if separated_by_hyphen {
    at += 1;
  }
  // without a separator there is no compound — "12A" is a suffixed number, not a pair.
  if !separated_by_hyphen && first_letter.is_none() {
    return None;
  }

  let second_digits = run_of_digits(&chars, &mut at);
  if second_digits.is_empty() {
    return None;
  }
  let second_letter = optional_letter(&chars, &mut at);
  if at != chars.len() {
    return None;
  }

  Some((
    part(&first_digits, first_letter),
    part(&second_digits, second_letter),
  ))
}

fn run_of_digits(chars: &[char], at: &mut usize) -> String {
  let start = *at;
  while chars.get(*at).is_some_and(char::is_ascii_digit) {
    *at += 1;
  }
  chars[start..*at].iter().collect()
}

fn optional_letter(chars: &[char], at: &mut usize) -> Option<char> {
  match chars.get(*at) {
    Some(c) if c.is_ascii_alphabetic() => {
      *at += 1;
      Some(*c)
    }
    _ => None,
  }
}

fn part(digits: &str, letter: Option<char>) -> String {
  match letter {
    Some(c) => format!("{digits}{}", c.to_ascii_uppercase()),
    None => digits.to_string(),
  }
}

fn digits_within(text: &str, shape: house_number_shape, max_digits: u8) -> bool {
  let max = max_digits as usize;
  match shape {
    house_number_shape::simple | house_number_shape::suffixed => {
      let digits = text.chars().take_while(|c| c.is_ascii_digit()).count();
      digits >= 1 && digits <= max
    }
    house_number_shape::compound => match split_compound(&text.to_ascii_uppercase()) {
      Some((first, second)) => [first, second].iter().all(|p| {
        let digits = p.chars().take_while(|c| c.is_ascii_digit()).count();
        digits >= 1 && digits <= max
      }),
      None => false,
    },
    house_number_shape::free => false,
  }
}

#[cfg(test)]
#[path = "value.test.rs"]
mod tests;
