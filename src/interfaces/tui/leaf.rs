use geo::Geometry;
use rusqlite::Connection;
use std::collections::HashMap;

use super::map::drawing;
use super::tree::{breadcrumb, entry, folder, item};
use crate::address::{admin_level, query_match};
use crate::admin_level::geometry::admin_geometry;
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

fn osm_field(own: &admin_level) -> Option<(&'static str, String)> {
  if let Some(ways) = &own.osm_merged_way_ids {
    let ways: Vec<String> = ways.iter().map(u64::to_string).collect();
    return Some(("osm ways", ways.join(", ")));
  }
  own
    .osm_way_id
    .map(|way| ("osm way", way.to_string()))
    .or_else(|| {
      own
        .osm_relation_id
        .map(|relation| ("osm relation", relation.to_string()))
    })
}

impl leaf {
  pub fn breadcrumb(&self) -> String {
    breadcrumb(&self.path)
  }

  pub fn fields(&self) -> Vec<(&'static str, String)> {
    let address = &self.address;
    let Some((own, above)) = address.admin_levels.split_last() else {
      return Vec::new();
    };
    let levels: Vec<&str> = above.iter().map(|level| level.name.as_str()).collect();
    let optional = [
      ("post code", address.attributes.post_code.clone()),
      (
        "country",
        address.attributes.country_iso_3166_1_alpha_2_code.clone(),
      ),
    ];
    let mut fields = vec![
      ("name", own.name.clone()),
      ("label", address.friendly_name.clone()),
    ];
    fields.extend(
      optional
        .into_iter()
        .filter_map(|(name, value)| Some((name, value?))),
    );
    fields.push((
      "point",
      format!("{:.5}, {:.5}", address.latitude, address.longitude),
    ));
    fields.extend(osm_field(own));
    fields.push(("id", address.id.clone()));
    if !levels.is_empty() {
      fields.push(("levels", levels.join(" / ")));
    }
    fields
  }
}
