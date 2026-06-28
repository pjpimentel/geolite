use geo::{Contains, Point};
use geozero::ToWkt;
use serde::Serialize;
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::extract::admin_levels::osm_admin_level;

pub mod address;
pub mod coordinates;
pub mod house_number;

#[derive(Serialize, ToSchema)]
pub(crate) enum query_service {
  coordinates_to_address,
  text_to_address,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct admin_level {
  pub(crate) level: u8,
  pub(crate) name: String,
  pub(crate) osm_relation_id: Option<u64>,
  pub(crate) osm_way_id: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) wkt: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct query_match_attributes {
  pub(crate) country_iso_3166_1_alpha_2_code: Option<String>,
  pub(crate) post_code: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(crate) enum house_number_match {
  exact,
  interpolated,
  absent, // TODO: precisa?
}

#[derive(Serialize, ToSchema)]
pub(crate) struct query_house_number {
  pub(crate) number: String,
  pub(crate) kind: house_number_match,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct query_match {
  pub(crate) admin_levels: Vec<admin_level>,
  pub(crate) latitude: f64,
  pub(crate) longitude: f64,
  pub(crate) coordinates_distance_in_meters: Option<u32>,
  pub(crate) similarity: Option<f32>,
  pub(crate) score: Option<f32>,
  pub(crate) friendly_name: String,
  pub(crate) attributes: query_match_attributes,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) house_number: Option<query_house_number>,
  pub(crate) id: u64,
  #[serde(skip)]
  pub(crate) admin_level_id: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct query_output {
  pub(crate) service: query_service,
  pub(crate) matches: Vec<query_match>,
}

// axis-aligned box. tipo interno: envelope de um bounding_geometry e params do rtree.
#[derive(Clone, Copy)]
pub(crate) struct bounding_box {
  pub(crate) min_lat: f64,
  pub(crate) max_lat: f64,
  pub(crate) min_lon: f64,
  pub(crate) max_lon: f64,
}

// filtro espacial arbitrario: a geometria (poligono/multipoligono) faz a contencao exata
// do ponto; o envelope (aabb) alimenta o pre-filtro grosso do rtree.
#[derive(Clone)]
pub(crate) struct bounding_geometry {
  pub(crate) geometry: geo::Geometry<f64>,
  pub(crate) envelope: bounding_box,
}

impl bounding_geometry {
  pub(crate) fn contains(&self, lat: f64, lon: f64) -> bool {
    self.geometry.contains(&Point::new(lon, lat))
  }

  #[cfg(test)]
  pub(crate) fn from_rect(b: bounding_box) -> Self {
    let ring = geo::LineString(vec![
      geo::Coord { x: b.min_lon, y: b.min_lat },
      geo::Coord { x: b.max_lon, y: b.min_lat },
      geo::Coord { x: b.max_lon, y: b.max_lat },
      geo::Coord { x: b.min_lon, y: b.max_lat },
      geo::Coord { x: b.min_lon, y: b.min_lat },
    ]);
    Self {
      geometry: geo::Geometry::Polygon(geo::Polygon::new(ring, vec![])),
      envelope: b,
    }
  }
}

pub(crate) const MAX_RESULTS: u8 = 10;
pub(crate) const MAX_FTS_HITS: u8 = 50;

// mapa id -> wkt para cada admin_level (folha + ancestrais). vazio quando desligado,
// evitando carregar geometrias (polygons de paises/estados sao grandes) no caminho comum.
pub(crate) fn load_wkt_by_ids(
  conn: &rusqlite::Connection,
  ids: &[i64],
  include_wkt: bool,
) -> HashMap<i64, String> {
  if !include_wkt {
    return HashMap::new();
  }
  crate::database::admin_levels::load_full_by_ids(conn, ids)
    .into_iter()
    .filter_map(|r| Some((r.id, r.wkb.as_ref()?.geometry().to_wkt().ok()?)))
    .collect()
}

// reference distance for the coordinate-quality score (matches the street snap
// reference in house_number.rs). 0m → 1.0, 100m → 0.0
const COORDINATE_QUALITY_REFERENCE_M: f64 = 100.0;

// dispatcher fino: cada sub-funcao retorna a saida ja filtrada e truncada (via
// apply_filters_and_truncate), garantindo que o truncate(MAX_RESULTS) seja o passo final
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
  conn: &rusqlite::Connection,
  tantivy_index: Option<&crate::index::admin_levels_hierarchy_tantivy::tantivy_index>,
  query: &str,
  friendly_name_format: Option<&str>,
  min_quality: Option<f64>,
  bounding_wkt: Option<bounding_geometry>,
  last_admin_levels: Option<Vec<u8>>,
  include_wkt: bool,
) -> query_output {
  if let Some((lat, lon)) = try_parse_coordinates(query) {
    coordinates::run(
      conn,
      lat,
      lon,
      friendly_name_format,
      bounding_wkt.as_ref(),
      min_quality,
      last_admin_levels.as_deref(),
      include_wkt,
    )
  } else {
    let index = tantivy_index.expect("can not query because index is unavailable");
    address::run(
      conn,
      index,
      query,
      friendly_name_format,
      last_admin_levels.as_deref(),
      bounding_wkt.as_ref(),
      min_quality,
      include_wkt,
    )
  }
}

// filtros removedores + truncate, sempre o ultimo passo de cada sub-funcao. o truncate
// vem depois de todos os filtros para nao descartar soluções validas ranqueadas alem do corte
pub(crate) fn apply_filters_and_truncate(
  matches: &mut Vec<query_match>,
  min_quality: Option<f64>,
  bounding_wkt: Option<&bounding_geometry>,
  last_admin_levels: Option<&[u8]>,
) {
  if let Some(threshold) = min_quality {
    matches.retain(|m| match_quality(m) >= threshold);
  }
  if let Some(b) = bounding_wkt {
    // o pre-filtro do rtree testa intersecao com o envelope, que nao implica que o ponto final
    // do match esteja dentro da geometria — esta contencao exata e a autoritativa
    matches.retain(|m| b.contains(m.latitude, m.longitude));
  }
  if let Some(levels) = last_admin_levels {
    matches.retain(|m| m.admin_levels.last().is_some_and(|a| levels.contains(&a.level)));
  }
  matches.truncate(MAX_RESULTS as usize);
}

// qualidade do match por distancia (caminho de coordenadas). 0m → 1.0, 100m → 0.0
pub(crate) fn coordinate_quality(distance_in_meters: Option<u32>) -> f64 {
  match distance_in_meters {
    Some(d) => (1.0 - d as f64 / COORDINATE_QUALITY_REFERENCE_M).clamp(0.0, 1.0),
    None => 1.0,
  }
}

fn match_quality(m: &query_match) -> f64 {
  if let Some(s) = m.similarity {
    return s as f64;
  }
  coordinate_quality(m.coordinates_distance_in_meters)
}

// accepted format: "<lat>,<lon>" with optional surrounding/inner whitespace.
// reverse order ("lon,lat") is not detected — there's no way to disambiguate
// without an out-of-band hint, so we pick the geographic convention.
pub(crate) fn try_parse_coordinates(s: &str) -> Option<(f64, f64)> {
  let s = s.trim();
  let (lat_str, lon_str) = s.split_once(',')?;
  let lat: f64 = lat_str.trim().parse().ok()?;
  let lon: f64 = lon_str.trim().parse().ok()?;
  if (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon) {
    Some((lat, lon))
  } else {
    None
  }
}

// builds friendly_name from admin_levels; street comes first, house number second,
// remaining levels ordered from most specific to least (higher level = more specific).
// used as fallback when no friendly_name_format template is provided
fn default_friendly_name(admin_levels: &[admin_level]) -> String {
  let street_level = osm_admin_level::street as u8;
  let house_level = osm_admin_level::house_numbers as u8;
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

enum template_segment {
  literal(String),
  placeholder(u8),
}

// scan format buffer for placeholders. two accepted forms:
//   `{admin_level_<N>_name}` where N parses as u8, and
//   `{house_number}` — a friendly alias for the house-number admin level.
// the parse is strict: any other `{...}` is a hard error, so a malformed template
// gives the user clear feedback instead of silently leaking braces into the output.
// callers must validate via `validate_friendly_name_format` at the input boundary
// before any render path runs (see `render_friendly_name`).
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
        osm_admin_level::house_numbers as u8
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

// single validation boundary for the friendly_name template; wired into the cli
// (`value_parser`) and the http `/geocode` handler so malformed templates are
// rejected with a descriptive error before any query runs.
pub(crate) fn validate_friendly_name_format(s: &str) -> Result<String, String> {
  parse_template(s)?;
  Ok(s.to_string())
}

// renders a template string against an admin_levels list.
// missing placeholders are dropped together with the literal that immediately follows
// them (so `"{a}, {b}, {c}"` with `b` missing renders as `"a, c"`, not `"a, , c"`).
// trailing/leading commas and whitespace left over after substitution are trimmed.
pub(crate) fn render_friendly_name(format: &str, admin_levels: &[admin_level]) -> String {
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

pub(super) fn round5(v: f64) -> f64 {
  (v * 100_000.0).round() / 100_000.0
}

#[cfg(test)]
#[path = "query.test.rs"]
mod tests;
