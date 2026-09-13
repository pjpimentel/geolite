use std::collections::HashMap;

use geozero::ToWkt;
use rusqlite::Connection;
use serde::Serialize;
use utoipa::ToSchema;

use super::label;
use crate::domain::admin_level::level;
use crate::domain::admin_level::repository::admin_meta_row;
use crate::domain::admin_level_hierarchy::hierarchy_lookup_row;

#[derive(Serialize, ToSchema)]
pub enum query_service {
  coordinates_to_address,
  text_to_address,
}

#[derive(Serialize, ToSchema)]
pub struct admin_level {
  pub level: u8,
  pub name: String,
  pub osm_relation_id: Option<u64>,
  pub osm_way_id: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub wkt: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct query_match_attributes {
  pub country_iso_3166_1_alpha_2_code: Option<String>,
  pub post_code: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub enum house_number_match {
  exact,
  interpolated,
  absent,
}

#[derive(Serialize, ToSchema)]
pub struct query_house_number {
  pub number: String,
  pub kind: house_number_match,
}

#[derive(Serialize, ToSchema)]
pub struct query_match {
  pub admin_levels: Vec<admin_level>,
  pub latitude: f64,
  pub longitude: f64,
  pub coordinates_distance_in_meters: Option<u32>,
  pub similarity: Option<f32>,
  pub score: Option<f32>,
  pub friendly_name: String,
  pub attributes: query_match_attributes,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub house_number: Option<query_house_number>,
  pub id: u64,
  #[serde(skip)]
  pub admin_level_id: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct query_output {
  pub service: query_service,
  pub matches: Vec<query_match>,
}

impl query_match {
  pub(super) fn append_house_number_level(&mut self, number: &str, friendly_name_format: Option<&str>) {
    self.admin_levels.push(admin_level {
      level: level::house_number.value(),
      name: number.to_string(),
      osm_relation_id: None,
      osm_way_id: None,
      wkt: None,
    });
    self.friendly_name = match friendly_name_format {
      Some(fmt) => label::render_friendly_name(fmt, &self.admin_levels),
      None => label::default_friendly_name(&self.admin_levels),
    };
  }
}

pub(super) fn round5(v: f64) -> f64 {
  (v * 100_000.0).round() / 100_000.0
}

pub(super) struct leaf<'a> {
  pub(super) id: i64,
  pub(super) level: level,
  pub(super) name: &'a str,
  pub(super) relation_id: Option<u64>,
  pub(super) way_id: Option<u64>,
}

pub(super) struct match_sources {
  pub(super) hierarchies: HashMap<i64, hierarchy_lookup_row>,
  pub(super) meta: HashMap<i64, admin_meta_row>,
  wkt: HashMap<i64, String>,
}

impl match_sources {
  pub(super) fn load(conn: &Connection, ids: &[i64], include_wkt: bool) -> Self {
    let hierarchies = crate::domain::admin_level_hierarchy::repository::load_by_ids(conn, ids);
    let mut meta_ids: Vec<i64> = ids.to_vec();
    meta_ids.extend(
      hierarchies
        .values()
        .flat_map(|h| h.ancestor_ids.iter().copied()),
    );
    meta_ids.sort_unstable();
    meta_ids.dedup();
    let meta = crate::domain::admin_level::repository::load_metadata_by_ids(conn, &meta_ids);
    // the polygons of countries and states are megabytes of wkt: nothing loads them unless asked
    let wkt = if include_wkt {
      load_wkt_by_ids(conn, &meta_ids)
    } else {
      HashMap::new()
    };
    match_sources {
      hierarchies,
      meta,
      wkt,
    }
  }

  pub(super) fn chain_of(&self, id: i64) -> &[i64] {
    self
      .hierarchies
      .get(&id)
      .map(|h| h.ancestor_ids.as_slice())
      .unwrap_or(&[])
  }

  pub(super) fn ancestors_of<'a>(
    &'a self,
    chain: impl Iterator<Item = &'a i64>,
  ) -> Vec<&'a admin_meta_row> {
    let mut ancestors: Vec<&admin_meta_row> = chain.filter_map(|id| self.meta.get(id)).collect();
    ancestors.sort_by_key(|a| a.admin_level);
    ancestors
  }

  pub(super) fn level_ladder(&self, ancestors: &[&admin_meta_row], leaf: &leaf) -> Vec<admin_level> {
    let mut admin_levels: Vec<admin_level> = ancestors
      .iter()
      .map(|a| admin_level {
        level: a.admin_level.value(),
        name: a.name.clone(),
        osm_relation_id: a.relation_id,
        osm_way_id: a.way_id,
        wkt: self.wkt.get(&a.id).cloned(),
      })
      .collect();
    admin_levels.push(admin_level {
      level: leaf.level.value(),
      name: leaf.name.to_string(),
      osm_relation_id: leaf.relation_id,
      osm_way_id: leaf.way_id,
      wkt: self.wkt.get(&leaf.id).cloned(),
    });
    admin_levels
  }
}

fn load_wkt_by_ids(conn: &Connection, ids: &[i64]) -> HashMap<i64, String> {
  crate::domain::admin_level::repository::load_full_by_ids(conn, ids)
    .into_iter()
    .filter_map(|r| Some((r.id, r.wkb.as_ref()?.geometry().to_wkt().ok()?)))
    .collect()
}

pub(super) fn friendly_name_of(
  format: Option<&str>,
  admin_levels: &[admin_level],
  hierarchy: Option<&hierarchy_lookup_row>,
  own_name: &str,
) -> String {
  match format {
    Some(fmt) => label::render_friendly_name(fmt, admin_levels),
    None => hierarchy
      .map(|h| h.user_friendly_name.clone())
      .unwrap_or_else(|| own_name.to_string()),
  }
}

pub(super) fn country_iso_of(
  ancestors: &[&admin_meta_row],
  leaf: Option<&admin_meta_row>,
) -> Option<String> {
  ancestors
    .iter()
    .copied()
    .chain(leaf)
    .find(|a| a.admin_level == level::country)
    .and_then(|a| a.country_iso_code.clone())
}

pub(super) fn post_code_of(ancestors: &[&admin_meta_row]) -> Option<String> {
  ancestors
    .iter()
    .filter(|a| a.post_code.is_some())
    .max_by_key(|a| a.admin_level)
    .and_then(|a| a.post_code.clone())
}

#[cfg(test)]
#[path = "entity.test.rs"]
mod tests;
