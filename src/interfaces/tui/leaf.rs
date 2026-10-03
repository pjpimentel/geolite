use geo::Geometry;
use rusqlite::Connection;
use std::collections::HashMap;

use super::fields::{field, listed, osm_element, piece};
use super::map::drawing;
use super::tree::{breadcrumb, entry, folder, item};
use crate::address::{admin_level, query_match};
use crate::admin_level::geometry::admin_geometry;
use crate::admin_level::level;
use crate::admin_level::repository::geometry_by_ids;

pub struct leaf {
  pub path: Vec<item>,
  pub address: query_match,
  pub drawing: drawing,
}

fn areas_above(folder: &folder) -> Vec<i64> {
  folder
    .path
    .iter()
    .rev()
    .filter_map(|item| match &item.entry {
      entry::place(node) => Some(node.id),
      entry::missing { .. } => None,
    })
    .collect()
}

pub fn open(conn: &Connection, folder: &folder, index: usize) -> Option<leaf> {
  let item = folder.items.get(index)?;
  let entry::place(node) = &item.entry else {
    return None;
  };
  let above = areas_above(folder);
  let address = crate::address::at_path(conn, node.id, &above)?;
  let drawn: Vec<i64> = std::iter::once(node.id)
    .chain(above.iter().copied())
    .collect();
  let geometries: HashMap<i64, admin_geometry> =
    geometry_by_ids(conn, &drawn).into_iter().collect();
  let areas: Vec<&Geometry<f64>> = above
    .iter()
    .filter_map(|area| geometries.get(area))
    .map(admin_geometry::geometry)
    .collect();
  let drawing = drawing::of(geometries.get(&node.id)?.geometry(), &areas)?;
  let mut path = folder.path.clone();
  path.push(item.clone());
  Some(leaf {
    path,
    address,
    drawing,
  })
}

fn osm_elements(level: &admin_level) -> Vec<osm_element> {
  if let Some(ways) = &level.osm_merged_way_ids {
    return ways.iter().copied().map(osm_element::way).collect();
  }
  level
    .osm_way_id
    .map(osm_element::way)
    .or(level.osm_relation_id.map(osm_element::relation))
    .into_iter()
    .collect()
}

fn kind_of(elements: &[osm_element]) -> Option<&'static str> {
  match elements {
    [] => None,
    [only] => Some(only.kind()),
    _ => Some("ways"),
  }
}

fn osm_field(own: &admin_level) -> Option<field> {
  let elements = osm_elements(own);
  let kind = kind_of(&elements)?;
  Some(field::of(&format!("osm {kind}"), listed(&elements)))
}

fn named(area: &admin_level) -> Vec<piece> {
  let elements = osm_elements(area);
  let name = &area.name;
  let Some(kind) = kind_of(&elements) else {
    return vec![piece::text(name.clone())];
  };
  let mut pieces = vec![piece::text(format!("{name} ({kind} "))];
  pieces.extend(listed(&elements));
  pieces.push(piece::text(")".to_string()));
  pieces
}

fn level_name(rung: u8) -> String {
  match level::new(rung) {
    Some(level) => format!("admin level {rung} ({})", level.name()),
    None => format!("admin level {rung}"),
  }
}

fn level_fields(rung: u8, path: &[admin_level]) -> Vec<field> {
  let name = level_name(rung);
  let areas: Vec<field> = path
    .iter()
    .filter(|area| area.level == rung)
    .map(|area| field::of(&name, named(area)))
    .collect();
  if areas.is_empty() {
    return vec![field::of(&name, vec![piece::absent])];
  }
  areas
}

fn rungs_of(path: &[admin_level]) -> Vec<u8> {
  let mut rungs: Vec<u8> = (0..=level::street.value())
    .filter_map(level::new)
    .map(level::value)
    .collect();
  rungs.extend(path.iter().map(|area| area.level));
  rungs.sort_unstable();
  rungs.dedup();
  rungs
}

impl leaf {
  pub fn breadcrumb(&self) -> String {
    breadcrumb(&self.path)
  }

  pub fn fields(&self) -> Vec<field> {
    let address = &self.address;
    let path = &address.admin_levels;
    let Some(own) = path.last() else {
      return Vec::new();
    };
    let optional = [
      ("post code", address.attributes.post_code.clone()),
      (
        "country code",
        address.attributes.country_iso_3166_1_alpha_2_code.clone(),
      ),
    ];
    let mut fields = vec![
      field::text("name", own.name.clone()),
      field::text("label", address.friendly_name.clone()),
    ];
    fields.extend(
      optional
        .into_iter()
        .filter_map(|(name, value)| Some(field::text(name, value?))),
    );
    fields.push(field::text(
      "point",
      format!("{:.5}, {:.5}", address.latitude, address.longitude),
    ));
    fields.extend(osm_field(own));
    fields.push(field::text("id", address.id.clone()));
    let rungs = rungs_of(path);
    fields.extend(rungs.into_iter().flat_map(|rung| level_fields(rung, path)));
    fields
  }
}
