use std::collections::{HashMap, HashSet};

use geo::Centroid;
use rusqlite::Connection;

use super::entity::{
  self, leaf, match_sources, query_match, query_match_attributes, query_output, query_service,
  round5,
};
use super::{filter, house_number, query_opts};
use crate::domain::admin_level::level;
use crate::domain::admin_level::repository::{admin_area_row, admin_meta_row};
use crate::domain::admin_level_hierarchy::search_index::{
  build_entity_text, tantivy_index, tokenize,
};
use crate::domain::house_number::house_number_policy;

const MAX_FTS_HITS: u8 = 50;

pub(super) fn run(
  conn: &Connection,
  house_numbers: &house_number_policy,
  index: &tantivy_index,
  text: &str,
  opts: &query_opts,
) -> query_output {
  let empty = || query_output {
    service: query_service::text_to_address,
    matches: vec![],
  };
  // a region restricts the ranking inside tantivy rather than filtering after the fts cut, so
  // a match of the region ranked below the global cap is not lost; the exact containment of the
  // polygon runs in the filters
  let region_ids = opts
    .bounding
    .as_ref()
    .map(|b| crate::domain::admin_level::repository::ids_in_bounding_box(conn, b.envelope));
  if region_ids.as_ref().is_some_and(|r| r.is_empty()) {
    return empty();
  }
  // the fts runs on the full text: a number can be part of the street name ("25" in
  // "rua 25 de marco"), so stripping it would break the match; the house number is resolved
  // per candidate afterwards
  let hits = index.search(
    text,
    MAX_FTS_HITS as usize,
    opts.last_admin_levels.as_deref(),
    region_ids.as_deref(),
  );
  if hits.is_empty() {
    return empty();
  }

  let query_tokens = tokenize(text);
  let ids: Vec<i64> = hits.iter().map(|(id, _)| *id).collect();
  let scores: HashMap<i64, f32> = hits.into_iter().collect();
  let sources = match_sources::load(conn, &ids, opts.include_wkt);
  let records = crate::domain::admin_level::repository::load_full_by_ids(conn, &ids);
  let record_map: HashMap<i64, &admin_area_row> = records.iter().map(|r| (r.id, r)).collect();

  let mut matches: Vec<query_match> = ids
    .iter()
    .filter_map(|id| {
      let record = record_map.get(id)?;
      let score = *scores.get(id).unwrap_or(&0.0);
      build_match(record, &sources, &query_tokens, score, opts)
    })
    .collect();

  // the house number moves the point and nudges the similarity that the filters read
  house_number::enrich_house_number_from_query(
    conn,
    text,
    &mut matches,
    opts.friendly_name_format,
    house_numbers,
  );

  // bm25 first; among the segments of one street, which share a score, the one that placed the
  // number wins the tie through the similarity nudge
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

  filter::apply_filters_and_truncate(&mut matches, opts);

  query_output {
    service: query_service::text_to_address,
    matches,
  }
}

fn build_match(
  record: &admin_area_row,
  sources: &match_sources,
  query_tokens: &[String],
  score: f32,
  opts: &query_opts,
) -> Option<query_match> {
  let geom = record.wkb.as_ref()?.geometry();
  let centroid = geom.centroid()?;

  let hierarchy = sources.hierarchies.get(&record.id);
  let ancestors = sources.ancestors_of(sources.chain_of(record.id).iter());
  let leaf = leaf {
    id: record.id,
    level: record.admin_level,
    name: &record.name,
    relation_id: record.relation_id,
    way_id: record.way_id,
  };
  let admin_levels = sources.level_ladder(&ancestors, &leaf);
  let friendly_name = entity::friendly_name_of(
    opts.friendly_name_format,
    &admin_levels,
    hierarchy,
    &record.name,
  );
  let own_meta = sources.meta.get(&record.id);
  let coverage = token_coverage(
    query_tokens,
    &doc_text(
      &record.name,
      own_meta.and_then(|m| m.post_code.as_deref()),
      &ancestors,
    ),
  );

  Some(query_match {
    admin_levels,
    latitude: round5(centroid.y()),
    longitude: round5(centroid.x()),
    coordinates_distance_in_meters: None,
    similarity: Some(round5(coverage as f64) as f32),
    score: Some(score),
    friendly_name,
    attributes: query_match_attributes {
      country_iso_3166_1_alpha_2_code: entity::country_iso_of(&ancestors, own_meta),
      post_code: entity::post_code_of(&ancestors),
    },
    house_number: None,
    id: record.id as u64,
    admin_level_id: (record.admin_level == level::street).then_some(record.id),
  })
}

// the text the index holds for a document, rebuilt through the same pipeline as the build so
// that the tokens agree
fn doc_text(own_name: &str, own_post_code: Option<&str>, ancestors: &[&admin_meta_row]) -> String {
  let mut out = build_entity_text(own_name, own_post_code);
  for a in ancestors {
    out.push(' ');
    out.push_str(&build_entity_text(&a.name, a.post_code.as_deref()));
  }
  out
}

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

#[cfg(test)]
#[path = "text.test.rs"]
mod tests;
