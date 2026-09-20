use std::collections::{BTreeSet, HashMap};

use geo::Point;
use rusqlite::Connection;
use serde::Serialize;
use utoipa::ToSchema;

use super::label;
use crate::admin_level::geometry::bounding_box;
use crate::admin_level::level;
use crate::admin_level::repository::admin_meta_row;
use crate::admin_level_hierarchy::paths::paths_of;

#[derive(Serialize, ToSchema)]
pub enum query_service {
  coordinates_to_address,
  text_to_address,
}

#[derive(Serialize, ToSchema)]
pub struct admin_level {
  pub level: u8,
  pub name: String,
  pub post_code: Option<String>,
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
  pub id: String,
}

#[derive(Serialize, ToSchema)]
pub struct query_output {
  pub service: query_service,
  pub matches: Vec<query_match>,
}

pub(super) fn round5(v: f64) -> f64 {
  (v * 100_000.0).round() / 100_000.0
}

// one area reached through two paths is two answers, so the identity of a match is its path and
// not its area: the ids from the root down to the leaf, the way a directory path reads
pub(super) fn path_id(own_id: i64, path: &[i64]) -> String {
  // uuid v5 of "https://github.com/pjpimentel/geolite" under the url namespace, computed once:
  // `new_v5` is not const, and the namespace never changes
  const NAMESPACE: uuid::Uuid = uuid::Uuid::from_u128(0x4d29_6f1c_5a5f_5b2e_9b8a_2f7d_3c61_8e04);

  let mut name = String::with_capacity((path.len() + 1) * 12);
  for id in path.iter().rev() {
    name.push_str(&id.to_string());
    name.push('/');
  }
  name.push_str(&own_id.to_string());
  uuid::Uuid::new_v5(&NAMESPACE, name.as_bytes()).to_string()
}

pub(super) struct leaf<'a> {
  pub(super) id: i64,
  pub(super) level: level,
  pub(super) name: &'a str,
  pub(super) relation_id: Option<u64>,
  pub(super) way_id: Option<u64>,
}

pub(super) struct match_sources {
  paths: HashMap<i64, Vec<Vec<i64>>>,
  pub(super) meta: HashMap<i64, admin_meta_row>,
  wkt: HashMap<i64, String>,
  boxes: HashMap<i64, bounding_box>,
}

impl match_sources {
  pub(super) fn load(conn: &Connection, ids: &[i64], include_wkt: bool) -> Self {
    let edges = crate::admin_level_hierarchy::repository::ancestry_of(conn, ids);
    let paths: HashMap<i64, Vec<Vec<i64>>> =
      ids.iter().map(|&id| (id, paths_of(id, &edges))).collect();
    let mut meta_ids: Vec<i64> = ids.to_vec();
    meta_ids.extend(paths.values().flatten().flatten().copied());
    meta_ids.sort_unstable();
    meta_ids.dedup();
    let meta = crate::admin_level::repository::load_metadata_by_ids(conn, &meta_ids);
    // the polygons of countries and states are megabytes of wkt: nothing loads them unless asked
    let wkt = if include_wkt {
      crate::admin_level::repository::wkt_by_ids(conn, &meta_ids)
    } else {
      HashMap::new()
    };
    match_sources {
      paths,
      meta,
      wkt,
      boxes: HashMap::new(),
    }
  }

  pub(super) fn load_leaf_boxes(&mut self, conn: &Connection) {
    let leaves: BTreeSet<i64> = self
      .paths
      .values()
      .filter(|paths| paths.len() > 1)
      .flatten()
      .filter_map(|path| path.first().copied())
      .collect();
    let leaves: Vec<i64> = leaves.into_iter().collect();
    self.boxes = crate::admin_level::repository::boxes_by_ids(conn, &leaves);
  }

  pub(super) fn center_of(&self, id: i64) -> Option<Point<f64>> {
    self.boxes.get(&id).map(bounding_box::center)
  }

  pub(super) fn box_covers(&self, id: i64, point: &Point<f64>) -> bool {
    self.boxes.get(&id).is_some_and(|mbr| mbr.covers(point))
  }

  // an area the hierarchy does not know still answers one match, with an empty path
  pub(super) fn paths_of(&self, id: i64) -> &[Vec<i64>] {
    const NO_PATH: &[Vec<i64>] = &[Vec::new()];

    self
      .paths
      .get(&id)
      .filter(|paths| !paths.is_empty())
      .map(Vec::as_slice)
      .unwrap_or(NO_PATH)
  }

  pub(super) fn path_at(&self, id: i64, ordinal: u8) -> Option<&[i64]> {
    self
      .paths
      .get(&id)
      .and_then(|paths| paths.get(ordinal as usize))
      .map(Vec::as_slice)
  }

  pub(super) fn ancestors_of<'a>(
    &'a self,
    chain: impl Iterator<Item = &'a i64>,
  ) -> Vec<&'a admin_meta_row> {
    let mut ancestors: Vec<&admin_meta_row> = chain.filter_map(|id| self.meta.get(id)).collect();
    ancestors.sort_by_key(|a| a.admin_level);
    ancestors
  }

  // the label without a template follows the chain's own order, not the ladder's: the ladder
  // sorts by level and the two services order a level differently, while the label is one
  pub(super) fn default_label(
    &self,
    own_id: i64,
    own_name: &str,
    house_number: Option<&str>,
    chain: &[i64],
  ) -> String {
    let own_post_code = self.meta.get(&own_id).and_then(|m| m.post_code.as_deref());
    label::place_label(
      (own_name, own_post_code),
      house_number,
      chain
        .iter()
        .filter_map(|id| self.meta.get(id))
        .map(|a| (a.name.as_str(), a.post_code.as_deref())),
    )
  }

  pub(super) fn post_code_of(&self, own_id: i64, chain: &[i64]) -> Option<String> {
    std::iter::once(&own_id)
      .chain(chain)
      .filter_map(|id| self.meta.get(id))
      .find_map(|m| m.post_code.clone())
  }

  pub(super) fn level_ladder(
    &self,
    ancestors: &[&admin_meta_row],
    leaf: &leaf,
    house_number: Option<&str>,
  ) -> Vec<admin_level> {
    let mut admin_levels: Vec<admin_level> = ancestors
      .iter()
      .map(|a| admin_level {
        level: a.admin_level.value(),
        name: a.name.clone(),
        post_code: a.post_code.clone(),
        osm_relation_id: a.relation_id,
        osm_way_id: a.way_id,
        wkt: self.wkt.get(&a.id).cloned(),
      })
      .collect();
    admin_levels.push(admin_level {
      level: leaf.level.value(),
      name: leaf.name.to_string(),
      post_code: self.meta.get(&leaf.id).and_then(|m| m.post_code.clone()),
      osm_relation_id: leaf.relation_id,
      osm_way_id: leaf.way_id,
      wkt: self.wkt.get(&leaf.id).cloned(),
    });
    if let Some(number) = house_number {
      admin_levels.push(admin_level {
        level: level::house_number.value(),
        name: number.to_string(),
        post_code: None,
        osm_relation_id: None,
        osm_way_id: None,
        wkt: None,
      });
    }
    admin_levels
  }
}

pub(super) fn friendly_name_of(
  format: Option<&str>,
  admin_levels: &[admin_level],
  sources: &match_sources,
  own_id: i64,
  own_name: &str,
  house_number: Option<&str>,
  chain: &[i64],
) -> String {
  match format {
    Some(fmt) => label::render_friendly_name(fmt, admin_levels),
    None => sources.default_label(own_id, own_name, house_number, chain),
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
