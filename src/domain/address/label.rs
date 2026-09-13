use super::entity::admin_level;
use crate::domain::admin_level::level;

enum template_segment {
  literal(String),
  placeholder(u8),
}

fn parse_template(format: &str) -> Result<Vec<template_segment>, String> {
  let mut segments: Vec<template_segment> = Vec::new();
  let mut buf = String::new();
  let bytes = format.as_bytes();
  let mut i = 0;
  while i < bytes.len() {
    if bytes[i] == b'{' {
      let Some(rel_close) = format[i..].find('}') else {
        return Err(
          "friendly_name_format: unterminated placeholder (missing closing '}')".to_string(),
        );
      };
      let inner = &format[i + 1..i + rel_close];
      let level = if inner == "house_number" {
        level::house_number.value()
      } else {
        match inner
          .strip_prefix("admin_level_")
          .and_then(|s| s.strip_suffix("_name"))
        {
          Some(mid) => mid.parse::<u8>().map_err(|_| {
            format!("friendly_name_format: invalid admin level '{mid}' (expected an integer 0-255)")
          })?,
          None => {
            return Err(format!(
              "friendly_name_format: unknown field '{inner}' (expected 'admin_level_<N>_name' or 'house_number')"
            ));
          }
        }
      };
      if !buf.is_empty() {
        segments.push(template_segment::literal(std::mem::take(&mut buf)));
      }
      segments.push(template_segment::placeholder(level));
      i += rel_close + 1;
      continue;
    }
    let ch = format[i..].chars().next().unwrap();
    buf.push(ch);
    i += ch.len_utf8();
  }
  if !buf.is_empty() {
    segments.push(template_segment::literal(buf));
  }
  Ok(segments)
}

pub fn validate_friendly_name_format(s: &str) -> Result<String, String> {
  parse_template(s)?;
  Ok(s.to_string())
}

pub(super) fn render_friendly_name(format: &str, admin_levels: &[admin_level]) -> String {
  let segments = parse_template(format)
    .expect("friendly_name_format must be validated at the parse boundary before render");
  let mut out = String::new();
  let mut skip_next_literal = false;
  for seg in segments {
    match seg {
      template_segment::literal(s) => {
        if skip_next_literal {
          skip_next_literal = false;
        } else {
          out.push_str(&s);
        }
      }
      template_segment::placeholder(level) => {
        match admin_levels.iter().find(|a| a.level == level) {
          Some(a) => {
            out.push_str(&a.name);
            skip_next_literal = false;
          }
          None => {
            skip_next_literal = true;
          }
        }
      }
    }
  }
  out
    .trim_matches(|c: char| c == ',' || c.is_whitespace())
    .to_string()
}

pub(super) fn default_friendly_name(admin_levels: &[admin_level]) -> String {
  let street_level = level::street.value();
  let house_level = level::house_number.value();
  let mut sorted: Vec<&admin_level> = admin_levels.iter().collect();
  sorted.sort_by_key(|a| {
    if a.level == street_level {
      (0u8, 0u8)
    } else if a.level == house_level {
      (1, 0)
    } else {
      (2, 255u8.saturating_sub(a.level))
    }
  });
  sorted
    .iter()
    .map(|a| a.name.as_str())
    .collect::<Vec<_>>()
    .join(", ")
}

#[cfg(test)]
#[path = "label.test.rs"]
mod tests;
