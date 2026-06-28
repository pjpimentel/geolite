use super::house_number::enrich_house_numbers;
use geo::{Closest, ClosestPoint, Geometry, HaversineDistance, LineString, Point};
use rusqlite::Connection;

const RTREE_DELTA_DEG: f64 = 0.1;

// sem filtro explícito casamos em qualquer lugar: as cláusulas de bbox viram sempre-verdadeiras
// e o pré-filtro do rtree continua sendo só o box do delta ao redor da coordenada
const WORLD_BOUNDING_BOX: super::bounding_box = super::bounding_box {
  min_lat: -90.0,
  max_lat: 90.0,
  min_lon: -180.0,
  max_lon: 180.0,
};

struct admin_candidate {
  id: i64,
  admin_level: u8,
  closest_point: Point<f64>,
  distance_in_meters: Option<u32>,
}

fn best_admin_levels(
  conn: &Connection,
  input_pt: Point<f64>,
  bounding_wkt: Option<&super::bounding_geometry>,
) -> Vec<admin_candidate> {
  let (lon, lat) = (input_pt.x(), input_pt.y());
  let envelope = bounding_wkt.map(|b| b.envelope).unwrap_or(WORLD_BOUNDING_BOX);
  let raw = crate::database::admin_levels::streets_for_coordinates(
    conn,
    lon,
    lat,
    RTREE_DELTA_DEG,
    envelope,
  );
  crate::debug!("debug: rtree raw={} for lon={} lat={}", raw.len(), lon, lat);

  let mut rej_wkt_none = 0;
  let mut rej_not_linestring = 0;
  let mut rej_empty = 0;
  let mut rej_indeterminate = 0;
  let mut rej_outside_polygon = 0;
  let mut min_dist = f64::MAX;

  let mut candidates: Vec<admin_candidate> = Vec::new();
  for s in raw {
    let geom = match s.wkb {
      Some(g) => g.into_geometry(),
      None => {
        rej_wkt_none += 1;
        continue;
      }
    };
    let linestrings: Vec<LineString<f64>> = match geom {
      Geometry::LineString(ls) => vec![ls],
      Geometry::MultiLineString(mls) => mls.0,
      _ => {
        rej_not_linestring += 1;
        continue;
      }
    };
    let mut best: Option<(Point<f64>, f64)> = None;
    let mut had_non_empty = false;
    for ls in &linestrings {
      if ls.0.is_empty() {
        continue;
      }
      had_non_empty = true;
      if let Closest::SinglePoint(p) | Closest::Intersection(p) = ls.closest_point(&input_pt) {
        let d = input_pt.haversine_distance(&p);
        if best.is_none_or(|(_, bd)| d < bd) {
          best = Some((p, d));
        }
      }
    }
    if !had_non_empty {
      rej_empty += 1;
      continue;
    }
    let (cp, dist) = match best {
      Some(b) => b,
      None => {
        rej_indeterminate += 1;
        continue;
      }
    };
    // contencao exata do polígono nos candidatos, antes da carga pesada — o enrich do
    // caminho de coordenadas nao move o ponto, entao filtrar aqui equivale a filtrar no final
    if let Some(b) = bounding_wkt
      && !b.contains(cp.y(), cp.x())
    {
      rej_outside_polygon += 1;
      continue;
    }
    if dist < min_dist {
      min_dist = dist;
    }
    let distance_in_meters = Some(dist.round().clamp(0.0, u32::MAX as f64) as u32);
    candidates.push(admin_candidate {
      id: s.id,
      admin_level: s.admin_level,
      closest_point: cp,
      distance_in_meters,
    });
  }

  crate::debug!(
    "debug: rejections wkt_none={} not_linestring={} empty={} indeterminate={} outside_polygon={} min_dist_m={:.2}",
    rej_wkt_none,
    rej_not_linestring,
    rej_empty,
    rej_indeterminate,
    rej_outside_polygon,
    min_dist
  );

  candidates.sort_by(|a, b| {
    b.admin_level
      .cmp(&a.admin_level)
      .then(a.distance_in_meters.cmp(&b.distance_in_meters))
  });
  candidates
}

/*
 * it should be simple:
 * 1. find best solutions, where best is: highest admin level and lowest distance.
 * 2. for each solution, load hierarchy
 * 3. for each solution, find best house number, where best is: closest.
 */
#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
  conn: &Connection,
  lat: f64,
  lon: f64,
  friendly_name_format: Option<&str>,
  bounding_wkt: Option<&super::bounding_geometry>,
  min_quality: Option<f64>,
  last_admin_levels: Option<&[u8]>,
  include_wkt: bool,
) -> super::query_output {
  let input_pt = Point::new(lon, lat);

  // 0 find the n best admin_levels where
  // a) highest admin level is better (street > neighborhood > city > etc.)
  // b) less distance is better
  let mut candidates = best_admin_levels(conn, input_pt, bounding_wkt);
  if let Some(threshold) = min_quality {
    candidates.retain(|c| super::coordinate_quality(c.distance_in_meters) >= threshold);
  }
  // só truncamos cedo (evitando carga pesada de candidatos descartados) quando nenhum filtro
  // pós-enrich resta. last_admin_levels depende do leaf pós-enrich (nível 30), então fica pro final
  if last_admin_levels.is_none() {
    candidates.truncate(super::MAX_RESULTS as usize);
  }
  crate::debug!(
    "debug: candidates={} raw_query=({}, {})",
    candidates.len(),
    lat,
    lon
  );
  let candidate_ids: Vec<i64> = candidates.iter().map(|c| c.id).collect();

  // 1 find the pre-computed hierarchy for each possible solution
  let hierarchies = crate::database::admin_levels_hierarchy::load_by_ids(conn, &candidate_ids);
  let mut all_ancestor_ids: Vec<i64> = hierarchies
    .values()
    .flat_map(|h| h.ancestor_ids.iter().copied())
    .collect();
  all_ancestor_ids.sort_unstable();
  all_ancestor_ids.dedup();

  // 2 enrich each solution with all the data
  let mut meta_ids = candidate_ids.clone();
  meta_ids.extend(all_ancestor_ids.iter().copied());
  meta_ids.sort_unstable();
  meta_ids.dedup();
  let meta_map = crate::database::admin_levels::load_metadata_by_ids(conn, &meta_ids);
  let wkt_by_id = super::load_wkt_by_ids(conn, &meta_ids, include_wkt);

  let mut matches: Vec<super::query_match> = Vec::new();
  for c in &candidates {
    let cp = c.closest_point;

    let candidate_meta = meta_map.get(&c.id);
    let candidate_name = candidate_meta.map(|m| m.name.clone()).unwrap_or_default();

    let hierarchy = hierarchies.get(&c.id);
    let base_name = hierarchy
      .map(|h| h.user_friendly_name.clone())
      .unwrap_or_else(|| candidate_name.clone());
    let ancestor_ids = hierarchy.map(|h| h.ancestor_ids.as_slice()).unwrap_or(&[]);

    // ancestor_ids vem do hierarchy index como "mais especifico → mais geral" (ex.: [c, b, a]).
    // invertemos antes do sort estavel para que dentro do mesmo admin_level a ordem fique
    // "mais geral → mais especifico" (ex.: [a, b, c]) sem afetar a ordem entre niveis distintos.
    let mut ancestors_sorted: Vec<&crate::database::admin_levels::admin_meta_row> = ancestor_ids
      .iter()
      .rev()
      .filter_map(|id| meta_map.get(id))
      .collect();
    ancestors_sorted.sort_by_key(|a| a.admin_level);

    let mut admin_levels: Vec<super::admin_level> = ancestors_sorted
      .iter()
      .map(|a| super::admin_level {
        level: a.admin_level,
        name: a.name.clone(),
        osm_relation_id: a.relation_id,
        osm_way_id: a.way_id,
        wkt: wkt_by_id.get(&a.id).cloned(),
      })
      .collect();
    admin_levels.push(super::admin_level {
      level: c.admin_level,
      name: candidate_name.clone(),
      osm_relation_id: candidate_meta.and_then(|m| m.relation_id),
      osm_way_id: candidate_meta.and_then(|m| m.way_id),
      wkt: wkt_by_id.get(&c.id).cloned(),
    });

    let friendly_name = match friendly_name_format {
      Some(fmt) => super::render_friendly_name(fmt, &admin_levels),
      None => base_name.clone(),
    };

    let country_iso = ancestors_sorted
      .iter()
      .copied()
      .chain(candidate_meta)
      .find(|a| a.admin_level == crate::extract::admin_levels::osm_admin_level::country as u8)
      .and_then(|a| a.country_iso_code.clone());

    let post_code = ancestors_sorted
      .iter()
      .filter(|a| a.post_code.is_some())
      .max_by_key(|a| a.admin_level)
      .and_then(|a| a.post_code.clone())
      .or_else(|| candidate_meta.and_then(|m| m.post_code.clone()));

    matches.push(super::query_match {
      admin_levels,
      latitude: super::round5(cp.y()),
      longitude: super::round5(cp.x()),
      coordinates_distance_in_meters: c.distance_in_meters,
      similarity: None,
      score: None,
      friendly_name,
      attributes: super::query_match_attributes {
        country_iso_3166_1_alpha_2_code: country_iso,
        post_code,
      },
      house_number: None,
      id: c.id as u64,
      admin_level_id: Some(c.id),
    });
  }

  enrich_house_numbers(conn, input_pt, &mut matches, friendly_name_format);

  // truncate final: aplica last_admin_levels (leaf pós-enrich) e re-confirma bounding_wkt/min_quality
  super::apply_filters_and_truncate(&mut matches, min_quality, bounding_wkt, last_admin_levels);

  super::query_output {
    service: super::query_service::coordinates_to_address,
    matches,
  }
}

#[cfg(test)]
#[path = "coordinates.test.rs"]
mod tests;
