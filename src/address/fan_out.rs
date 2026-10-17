use std::cmp::Ordering;

use super::entity::{database_status, query_match, query_output, query_service};
use super::filter::MAX_RESULTS;
use super::{base, coordinates, query_opts, text};

// one base answers in its own order; several answer together, and what orders them has to
// compare across bases: the distance of a coordinate does, the bm25 of a text does not, so a text
// is ordered by the coverage of its tokens and the score only breaks the tie
pub(super) fn by_text(bases: &[base], text: &str, opts: &query_opts) -> query_output {
  let indexed: Vec<&base> = bases.iter().filter(|b| b.index.is_some()).collect();
  let answers = indexed.iter().map(|b| {
    text::run(
      b.conn,
      b.index.expect("the base was kept for its index"),
      text,
      opts,
    )
  });
  let matches = if indexed.len() == 1 {
    answers.into_iter().next().expect("one base answers")
  } else {
    let mut matches: Vec<query_match> = answers.flatten().collect();
    matches.sort_by(|a, b| {
      descending(b.similarity, a.similarity).then_with(|| descending(b.score, a.score))
    });
    matches.truncate(MAX_RESULTS as usize);
    matches
  };
  query_output {
    service: query_service::text_to_address,
    databases: databases_of(bases),
    matches,
  }
}

pub(super) fn by_coordinates(
  bases: &[base],
  latitude: f64,
  longitude: f64,
  opts: &query_opts,
) -> query_output {
  let answers = bases
    .iter()
    .map(|b| coordinates::run(b.conn, latitude, longitude, opts));
  let matches = if bases.len() == 1 {
    answers.into_iter().next().expect("one base answers")
  } else {
    let mut matches: Vec<query_match> = answers.flatten().collect();
    matches.sort_by(|a, b| {
      leaf_level(b)
        .cmp(&leaf_level(a))
        .then_with(|| {
          a.coordinates_distance_in_meters
            .cmp(&b.coordinates_distance_in_meters)
        })
        .then_with(|| a.id.cmp(&b.id))
    });
    matches.truncate(MAX_RESULTS as usize);
    matches
  };
  query_output {
    service: query_service::coordinates_to_address,
    databases: databases_of(bases),
    matches,
  }
}

fn databases_of(bases: &[base]) -> Vec<database_status> {
  bases
    .iter()
    .map(|b| database_status::of(b.file, b.conn, b.index))
    .collect()
}

fn leaf_level(m: &query_match) -> u8 {
  m.admin_levels.last().map_or(0, |a| a.level)
}

fn descending(a: Option<f32>, b: Option<f32>) -> Ordering {
  a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}
