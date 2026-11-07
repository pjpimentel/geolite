use std::fmt;

use crate::admin_level::id::{admin_level_id, osm_element_kind};

const SEPARATOR: char = '.';
const HOUSE_NUMBER_MARK: &str = ".n";

pub struct address_id {
  pub iso: String,
  pub leaf: i64,
  pub path: Vec<i64>,
  pub house_number: Option<String>,
}

impl address_id {
  pub fn encode(iso: &str, leaf: i64, path: &[i64], house_number: Option<&str>) -> String {
    let mut encoded = String::with_capacity(3 + (path.len() + 1) * 12);
    encoded.push_str(&iso.to_ascii_lowercase());
    for id in path.iter().rev().chain(std::iter::once(&leaf)) {
      encoded.push(SEPARATOR);
      push_element(&mut encoded, *id);
    }
    if let Some(number) = house_number {
      encoded.push_str(HOUSE_NUMBER_MARK);
      encoded.push_str(number);
    }
    encoded
  }

  pub fn parse(raw: &str) -> Option<Self> {
    let lowered = raw.trim().to_ascii_lowercase();
    let (elements, house_number) = match lowered.split_once(HOUSE_NUMBER_MARK) {
      Some((_, "")) => return None,
      Some((elements, number)) => (elements, Some(number.to_string())),
      None => (lowered.as_str(), None),
    };
    let mut segments = elements.split(SEPARATOR);
    let iso = segments.next()?;
    if iso.len() != 2 || !iso.bytes().all(|b| b.is_ascii_lowercase()) {
      return None;
    }
    let mut ids: Vec<i64> = Vec::new();
    for segment in segments {
      ids.push(parse_element(segment)?);
    }
    let leaf = ids.pop()?;
    ids.reverse();
    Some(address_id {
      iso: iso.to_string(),
      leaf,
      path: ids,
      house_number,
    })
  }
}

impl fmt::Display for address_id {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&Self::encode(
      &self.iso,
      self.leaf,
      &self.path,
      self.house_number.as_deref(),
    ))
  }
}

fn push_element(encoded: &mut String, id: i64) {
  let element = admin_level_id::from_raw(id as u64);
  encoded.push(match element.kind() {
    osm_element_kind::relation => 'r',
    osm_element_kind::way => 'w',
  });
  encoded.push_str(&element.osm_id().to_string());
}

fn parse_element(segment: &str) -> Option<i64> {
  let element_of: fn(u64) -> admin_level_id = match segment.as_bytes().first()? {
    b'r' => admin_level_id::from_relation,
    b'w' => admin_level_id::from_way,
    _ => return None,
  };
  let digits = &segment[1..];
  if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
    return None;
  }
  let osm_id: u64 = digits.parse().ok()?;
  if osm_id >> 62 != 0 {
    return None;
  }
  Some(element_of(osm_id).raw() as i64)
}
