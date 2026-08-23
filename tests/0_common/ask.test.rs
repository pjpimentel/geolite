#![allow(dead_code)]

use super::harness::{encode, get, server, world};
use serde_json::Value;

#[derive(Clone)]
pub struct ask {
  input: String,
  include_wkt: bool,
  friendly_name_format: Option<String>,
  min_quality: Option<String>,
  bounding_wkt: Option<String>,
  last_admin_levels: Option<String>,
}

pub fn ask(input: &str) -> ask {
  ask {
    input: input.to_string(),
    include_wkt: false,
    friendly_name_format: None,
    min_quality: None,
    bounding_wkt: None,
    last_admin_levels: None,
  }
}

impl ask {
  pub fn with_wkt(mut self) -> Self {
    self.include_wkt = true;
    self
  }

  pub fn friendly_name_format(mut self, value: &str) -> Self {
    self.friendly_name_format = Some(value.to_string());
    self
  }

  pub fn min_quality(mut self, value: &str) -> Self {
    self.min_quality = Some(value.to_string());
    self
  }

  pub fn bounding_wkt(mut self, value: &str) -> Self {
    self.bounding_wkt = Some(value.to_string());
    self
  }

  pub fn last_admin_levels(mut self, value: &str) -> Self {
    self.last_admin_levels = Some(value.to_string());
    self
  }

  pub fn cli_args(&self) -> Vec<String> {
    let mut args = vec![
      "query".to_string(),
      self.input.clone(),
      "--include-wkt".to_string(),
      self.include_wkt.to_string(),
    ];
    for (flag, value) in [
      ("--friendly-name-format", &self.friendly_name_format),
      ("--min-quality", &self.min_quality),
      ("--bounding-wkt", &self.bounding_wkt),
      ("--last-admin-levels", &self.last_admin_levels),
    ] {
      if let Some(value) = value {
        args.push(flag.to_string());
        args.push(value.clone());
      }
    }
    args
  }

  pub fn http_path(&self) -> String {
    let mut path = format!(
      "/geocode?query={}&include_wkt={}",
      encode(&self.input),
      self.include_wkt
    );
    for (param, value) in [
      ("friendly_name_format", &self.friendly_name_format),
      // the http surface calls it `quality`, the cli `--min-quality`.
      ("quality", &self.min_quality),
      ("bounding_wkt", &self.bounding_wkt),
      ("last_admin_levels", &self.last_admin_levels),
    ] {
      if let Some(value) = value {
        path.push_str(&format!("&{param}={}", encode(value)));
      }
    }
    path
  }
}

impl std::fmt::Display for ask {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self.input)?;
    for (flag, value) in [
      ("friendly_name_format", &self.friendly_name_format),
      ("min_quality", &self.min_quality),
      ("bounding_wkt", &self.bounding_wkt),
      ("last_admin_levels", &self.last_admin_levels),
    ] {
      if let Some(value) = value {
        write!(f, " {flag}={value}")?;
      }
    }
    if self.include_wkt {
      write!(f, " +wkt")?;
    }
    Ok(())
  }
}

pub fn contains(actual: &Value, expected: &Value) -> Result<(), String> {
  let mut problems = Vec::new();
  walk(actual, expected, "$", &mut problems);
  if problems.is_empty() {
    Ok(())
  } else {
    Err(problems.join("\n  "))
  }
}

fn walk(actual: &Value, expected: &Value, path: &str, problems: &mut Vec<String>) {
  match (actual, expected) {
    (Value::Object(a), Value::Object(e)) => {
      for (key, value) in e {
        match a.get(key) {
          Some(found) => walk(found, value, &format!("{path}.{key}"), problems),
          None => problems.push(format!("{path}.{key} is missing")),
        }
      }
    }
    (Value::Array(a), Value::Array(e)) => {
      for (i, value) in e.iter().enumerate() {
        match a.get(i) {
          Some(found) => walk(found, value, &format!("{path}[{i}]"), problems),
          None => problems.push(format!("{path}[{i}] is missing (only {} present)", a.len())),
        }
      }
    }
    _ => {
      if actual != expected {
        problems.push(format!("{path}: expected {expected}, got {actual}"));
      }
    }
  }
}

impl world {
  pub fn assert_cli(&self, ask: &ask, expected: &Value) -> Value {
    // cli_args() leads with "query"; query_json prepends it itself.
    let owned = ask.cli_args();
    let args: Vec<&str> = owned[1..].iter().map(String::as_str).collect();
    let actual = self.query_json(&args);
    if let Err(problems) = contains(&actual, expected) {
      panic!("cli disagreed for {ask}\n  {problems}\n\nfull response:\n{actual:#}");
    }
    actual
  }

  pub fn assert_http(&self, server: &server, ask: &ask, expected: &Value) -> Value {
    let response = get(server.port, &ask.http_path());
    assert_eq!(
      response.status,
      200,
      "http {ask} returned {}: {}",
      response.status,
      response.text()
    );
    let actual = response.json();
    if let Err(problems) = contains(&actual, expected) {
      panic!("http disagreed for {ask}\n  {problems}\n\nfull response:\n{actual:#}");
    }
    actual
  }

  pub fn assert_both(&self, server: &server, ask: &ask, expected: &Value) {
    let from_cli = self.assert_cli(ask, expected);
    let from_http = self.assert_http(server, ask, expected);
    assert_eq!(
      from_cli, from_http,
      "the cli and the http api disagreed for {ask}"
    );
  }
}
