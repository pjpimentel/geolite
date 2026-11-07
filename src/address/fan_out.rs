use std::cmp::Ordering;

use rusqlite::Connection;

use super::entity::{database_status, query_match, query_output, query_service};
use super::filter::MAX_RESULTS;
use super::{base, coordinates, query_opts, text};
use crate::admin_level::country_index::country_index;
use crate::admin_level_hierarchy::search_index::{search_hit, search_tier};
use crate::admin_level_hierarchy::tantivy_index;

// the three queries of the index walk across the bases, every base asked at once and the next
// query only when no base answered the one before: a street one base holds exactly costs the
// time of that base, and the fuzzy query of the others never runs. one base answers in its own
// order; several answer together, and what orders them has to compare across bases: the bm25 of
// a text does not, so a text is ordered by the coverage of its tokens and the score only breaks
// the tie
pub(super) fn by_text(bases: &mut [base], text: &str, opts: &query_opts) -> query_output {
  let databases = databases_of(bases);
  let mut indexed: Vec<&mut base> = bases.iter_mut().filter(|b| b.index.is_some()).collect();
  let regions: Vec<Option<Vec<i64>>> = if opts.bounding.is_some() {
    in_parallel(indexed.iter_mut().map(|b| &mut *b.conn).collect(), |conn| {
      text::region_of(conn, opts)
    })
  } else {
    indexed.iter().map(|_| None).collect()
  };
  let mut hits: Vec<Vec<search_hit>> = vec![];
  for tier in search_tier::WALK {
    let asked: Vec<(&tantivy_index, Option<&[i64]>)> = indexed
      .iter()
      .zip(&regions)
      .map(|(b, region)| (b.index.expect("the base was kept for its index"), region.as_deref()))
      .collect();
    hits = in_parallel(asked, |(index, region)| text::hits_at(index, tier, text, opts, region));
    if hits.iter().any(|h| !h.is_empty()) {
      break;
    }
  }
  let answering: Vec<(usize, &str, &mut Connection, &[search_hit])> = indexed
    .iter_mut()
    .zip(&hits)
    .enumerate()
    .filter(|(_, (_, h))| !h.is_empty())
    .map(|(i, (b, h))| (i, b.iso, &mut *b.conn, h.as_slice()))
    .collect();
  let answered = answering.len();
  let answers = in_parallel(answering, |(i, iso, conn, hits)| {
    (i, text::matches_of(conn, iso, text, hits, opts))
  });
  let mut tagged = tag_by_base(answers.into_iter());
  if answered > 1 {
    tagged.sort_by(|(_, a), (_, b)| {
      descending(b.similarity, a.similarity).then_with(|| descending(b.score, a.score))
    });
    tagged.truncate(MAX_RESULTS as usize);
  }
  query_output {
    service: query_service::text_to_address,
    databases,
    matches: tagged.into_iter().map(|(_, m)| m).collect(),
  }
}

// the base of the country whose boundary holds the point, the one its streets were written to:
// a point no base of the data path covers answers nothing and names no base
pub(super) fn by_coordinates(
  bases: &mut [base],
  latitude: f64,
  longitude: f64,
  opts: &query_opts,
) -> query_output {
  let country = country_index::shared().country_at(longitude, latitude);
  let holder = bases
    .iter_mut()
    .find(|b| country.is_some_and(|iso| b.iso.eq_ignore_ascii_case(iso)));
  let Some(holder) = holder else {
    return query_output {
      service: query_service::coordinates_to_address,
      databases: vec![],
      matches: vec![],
    };
  };
  let databases = vec![database_status::of(holder.file, holder.conn, holder.index)];
  let matches = coordinates::run(holder.conn, holder.iso, latitude, longitude, opts);
  query_output {
    service: query_service::coordinates_to_address,
    databases,
    matches,
  }
}

// one thread per base, the answers joined in the order of the bases; a lone base answers inline,
// as it did before a data path held several
pub(crate) fn in_parallel<I: Send, T: Send>(items: Vec<I>, f: impl Fn(I) -> T + Sync) -> Vec<T> {
  if items.len() < 2 {
    return items.into_iter().map(f).collect();
  }
  std::thread::scope(|scope| {
    let f = &f;
    let handles: Vec<_> = items
      .into_iter()
      .map(|item| scope.spawn(move || f(item)))
      .collect();
    handles
      .into_iter()
      .map(|handle| handle.join().expect("a base answers or panics"))
      .collect()
  })
}

fn tag_by_base(
  answers: impl Iterator<Item = (usize, Vec<query_match>)>,
) -> Vec<(usize, query_match)> {
  answers
    .flat_map(|(i, matches)| matches.into_iter().map(move |m| (i, m)))
    .collect()
}


fn databases_of(bases: &[base]) -> Vec<database_status> {
  bases
    .iter()
    .map(|b| database_status::of(b.file, b.conn, b.index))
    .collect()
}

fn descending(a: Option<f32>, b: Option<f32>) -> Ordering {
  a.partial_cmp(&b).unwrap_or(Ordering::Equal)
}
