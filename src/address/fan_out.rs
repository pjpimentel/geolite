use std::cmp::Ordering;
use std::collections::BTreeMap;

use rusqlite::Connection;

use super::entity::{attach_wkt, database_status, query_match, query_output, query_service};
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
  let mut tagged = tag_by_base(answers);
  if indexed.len() > 1 {
    tagged.sort_by(|(_, a), (_, b)| {
      descending(b.similarity, a.similarity).then_with(|| descending(b.score, a.score))
    });
    tagged.truncate(MAX_RESULTS as usize);
  }
  query_output {
    service: query_service::text_to_address,
    databases: databases_of(bases),
    matches: with_wkt(tagged, |i| indexed[i].conn, opts),
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
  let mut tagged = tag_by_base(answers);
  if bases.len() > 1 {
    tagged.sort_by(|(_, a), (_, b)| {
      leaf_level(b)
        .cmp(&leaf_level(a))
        .then_with(|| {
          a.coordinates_distance_in_meters
            .cmp(&b.coordinates_distance_in_meters)
        })
        .then_with(|| a.id.cmp(&b.id))
    });
    tagged.truncate(MAX_RESULTS as usize);
  }
  query_output {
    service: query_service::coordinates_to_address,
    databases: databases_of(bases),
    matches: with_wkt(tagged, |i| bases[i].conn, opts),
  }
}

fn tag_by_base(answers: impl Iterator<Item = Vec<query_match>>) -> Vec<(usize, query_match)> {
  answers
    .enumerate()
    .flat_map(|(i, matches)| matches.into_iter().map(move |m| (i, m)))
    .collect()
}

// the polygons of countries and states are megabytes of wkt: they are loaded for the matches that
// survived the cut, never for the candidates of every base
fn with_wkt<'a>(
  mut tagged: Vec<(usize, query_match)>,
  conn_of: impl Fn(usize) -> &'a Connection,
  opts: &query_opts,
) -> Vec<query_match> {
  if opts.include_wkt {
    let mut by_base: BTreeMap<usize, Vec<&mut query_match>> = BTreeMap::new();
    for (i, m) in &mut tagged {
      by_base.entry(*i).or_default().push(m);
    }
    for (i, mut matches) in by_base {
      attach_wkt(conn_of(i), &mut matches);
    }
  }
  tagged.into_iter().map(|(_, m)| m).collect()
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
