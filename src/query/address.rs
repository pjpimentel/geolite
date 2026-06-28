use geo::Centroid;
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

use crate::extract::admin_levels::osm_admin_level;
use crate::index::admin_levels_hierarchy_tantivy::{
  build_entity_text, tantivy_index, tokenize,
};

// retorna (admin_level_id, score) ordenado por score desc do tantivy.
// scoring é BM25 nativo, com boost 3x no campo `name` (proprio nome do entity)
// vs 1x no `hier` (ancestrais). matches fuzzy ate edit distance 2.
fn search_hits(
  tantivy_index: &tantivy_index,
  query: &str,
  last_admin_levels: Option<&[u8]>,
  allowed_ids: Option<&[i64]>,
) -> Vec<(i64, f32)> {
  tantivy_index.search(query, super::MAX_FTS_HITS as usize, last_admin_levels, allowed_ids)
}

// reconstrói o texto indexado pelo doc (name + postcode próprio + ancestrais
// com seus postcodes), pra contar coverage da query contra essa massa.
// precisa replicar o pipeline do build pra que as tokens batam exato.
fn doc_text(
  own_name: &str,
  own_post_code: Option<&str>,
  ancestors: &[&crate::database::admin_levels::admin_meta_row],
) -> String {
  let mut out = build_entity_text(own_name, own_post_code);
  for a in ancestors {
    out.push(' ');
    out.push_str(&build_entity_text(&a.name, a.post_code.as_deref()));
  }
  out
}

// coverage = fração de tokens da query (após mesmo pipeline do índice) que
// aparecem exatos na massa de tokens do doc. 1.0 = toda palavra digitada está
// presente. fuzzy do tantivy ajuda ordenação mas não conta aqui — coverage
// reflete só presença exata pra ser interpretável.
fn token_coverage(query_tokens: &[String], doc_text: &str) -> f32 {
  if query_tokens.is_empty() {
    return 0.0;
  }
  let doc_tokens: HashSet<String> = tokenize(doc_text).into_iter().collect();
  let hits = query_tokens
    .iter()
    .filter(|t| doc_tokens.contains(t.as_str()))
    .count();
  hits as f32 / query_tokens.len() as f32
}

fn build_match(
  record: &crate::database::admin_levels::admin_area_row,
  hierarchy: Option<&crate::database::admin_levels_hierarchy::hierarchy_lookup_row>,
  meta_map: &HashMap<i64, crate::database::admin_levels::admin_meta_row>,
  query_tokens: &[String],
  raw_score: f32,
  friendly_name_format: Option<&str>,
  wkt_by_id: &HashMap<i64, String>,
) -> Option<super::query_match> {
  let geom = record.wkb.as_ref()?.geometry();
  // geo::Centroid: linestring → ponderado por comprimento; polygon/multipolygon → ponderado por área
  let centroid = geom.centroid()?;

  let ancestor_ids = hierarchy.map(|h| h.ancestor_ids.as_slice()).unwrap_or(&[]);
  let mut ancestors: Vec<&crate::database::admin_levels::admin_meta_row> = ancestor_ids
    .iter()
    .filter_map(|id| meta_map.get(id))
    .collect();
  ancestors.sort_by_key(|a| a.admin_level);

  let mut admin_levels: Vec<super::admin_level> = ancestors
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
    level: record.admin_level,
    name: record.name.clone(),
    osm_relation_id: record.relation_id,
    osm_way_id: record.way_id,
    wkt: wkt_by_id.get(&record.id).cloned(),
  });

  let friendly_name = match friendly_name_format {
    Some(fmt) => super::render_friendly_name(fmt, &admin_levels),
    None => hierarchy
      .map(|h| h.user_friendly_name.clone())
      .unwrap_or_else(|| record.name.clone()),
  };

  let country_iso = ancestors
    .iter()
    .copied()
    .chain(meta_map.get(&record.id))
    .find(|a| a.admin_level == osm_admin_level::country as u8)
    .and_then(|a| a.country_iso_code.clone());
  let post_code = ancestors
    .iter()
    .filter(|a| a.post_code.is_some())
    .max_by_key(|a| a.admin_level)
    .and_then(|a| a.post_code.clone());

  let is_street = record.admin_level == osm_admin_level::street as u8;
  let admin_level_id = if is_street { Some(record.id) } else { None };

  let record_post_code = meta_map
    .get(&record.id)
    .and_then(|m| m.post_code.as_deref());
  let doc_text = doc_text(&record.name, record_post_code, &ancestors);
  let coverage = token_coverage(query_tokens, &doc_text);

  Some(super::query_match {
    admin_levels,
    latitude: super::round5(centroid.y()),
    longitude: super::round5(centroid.x()),
    coordinates_distance_in_meters: None,
    similarity: Some(super::round5(coverage as f64) as f32),
    score: Some(raw_score),
    friendly_name,
    attributes: super::query_match_attributes {
      country_iso_3166_1_alpha_2_code: country_iso,
      post_code,
    },
    house_number: None,
    id: record.id as u64,
    admin_level_id,
  })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
  conn: &Connection,
  tantivy_index: &tantivy_index,
  query: &str,
  friendly_name_format: Option<&str>,
  last_admin_levels: Option<&[u8]>,
  bounding_wkt: Option<&super::bounding_geometry>,
  min_quality: Option<f64>,
  include_wkt: bool,
) -> super::query_output {
  // the fts runs on the full text: a number can be part of the street name ("25" in
  // "rua 25 de marco"), so stripping would break the match. the house number is resolved
  // per candidate afterwards (first numeric token left after removing the street's own name).
  // espacial-primeiro: quando ha filtro, restringimos o ranking textual aos ids da regiao
  // (envelope) no proprio tantivy, em vez de filtrar depois do corte do fts — assim matches da
  // regiao ranqueados abaixo do teto global nao se perdem. a contencao exata do poligono fica
  // no apply_filters_and_truncate (refina envelope → poligono)
  let region_ids =
    bounding_wkt.map(|b| crate::database::admin_levels::ids_in_bounding_box(conn, b.envelope));
  if region_ids.as_ref().is_some_and(|r| r.is_empty()) {
    return super::query_output {
      service: super::query_service::text_to_address,
      matches: vec![],
    };
  }
  let hits = search_hits(tantivy_index, query, last_admin_levels, region_ids.as_deref());
  if hits.is_empty() {
    return super::query_output {
      service: super::query_service::text_to_address,
      matches: vec![],
    };
  }

  let query_tokens = tokenize(query);
  let ids: Vec<i64> = hits.iter().map(|(id, _)| *id).collect();
  let scores: HashMap<i64, f32> = hits.into_iter().collect();

  let hierarchies = crate::database::admin_levels_hierarchy::load_by_ids(conn, &ids);
  let mut all_ancestor_ids: Vec<i64> = hierarchies
    .values()
    .flat_map(|h| h.ancestor_ids.iter().copied())
    .collect();
  all_ancestor_ids.sort_unstable();
  all_ancestor_ids.dedup();

  let mut meta_ids = ids.clone();
  meta_ids.extend(all_ancestor_ids);
  meta_ids.sort_unstable();
  meta_ids.dedup();
  let meta_map = crate::database::admin_levels::load_metadata_by_ids(conn, &meta_ids);
  let wkt_by_id = super::load_wkt_by_ids(conn, &meta_ids, include_wkt);

  let records = crate::database::admin_levels::load_full_by_ids(conn, &ids);
  let record_map: HashMap<i64, &crate::database::admin_levels::admin_area_row> =
    records.iter().map(|r| (r.id, r)).collect();

  // ordem é a do tantivy (BM25 desc da query que efetivamente achou os docs:
  // strict se ela retornou hits, loose caso contrário). similarity = coverage
  // (0-1). score = BM25 cru, sem transformações.
  let mut matches: Vec<super::query_match> = ids
    .iter()
    .filter_map(|id| {
      let record = record_map.get(id)?;
      let hierarchy = hierarchies.get(id);
      let raw_score = *scores.get(id).unwrap_or(&0.0);
      build_match(
        record,
        hierarchy,
        &meta_map,
        &query_tokens,
        raw_score,
        friendly_name_format,
        &wkt_by_id,
      )
    })
    .collect();

  // enrich antes dos filtros: o house_number move lat/lon (pro ponto do numero) e ajusta
  // similarity, que os filtros bounding_wkt/min_quality leem. truncate fica por ultimo.
  super::house_number::enrich_house_number_from_query(conn, query, &mut matches, friendly_name_format);

  // keep bm25 (score) as the primary order; break ties by similarity, so among two segments of
  // the same street with the same score the one that resolved the house number (which gets a
  // small similarity nudge) ranks first
  matches.sort_by(|a, b| {
    b.score
      .partial_cmp(&a.score)
      .unwrap_or(std::cmp::Ordering::Equal)
      .then_with(|| {
        b.similarity
          .partial_cmp(&a.similarity)
          .unwrap_or(std::cmp::Ordering::Equal)
      })
  });

  super::apply_filters_and_truncate(&mut matches, min_quality, bounding_wkt, last_admin_levels);

  super::query_output {
    service: super::query_service::text_to_address,
    matches,
  }
}

#[cfg(test)]
#[path = "text_search.test.rs"]
mod text_search_test;
