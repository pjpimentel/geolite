use rusqlite::Connection;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tantivy::{
  Index, IndexReader, Searcher, TantivyDocument, Term,
  collector::TopDocs,
  query::{
    BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, PhraseQuery, Query, TermQuery, TermSetQuery,
  },
  schema::{
    Field, INDEXED, IndexRecordOption, STORED, Schema, TextFieldIndexing, TextOptions, Value,
  },
  tokenizer::{AsciiFoldingFilter, LowerCaser, SimpleTokenizer, TextAnalyzer},
};

use super::entity::decode_chain;
use super::repository;
use crate::domain::admin_level::level;

const TOKENIZER_NAME: &str = "geolite_ascii";
const TOKENIZER_STRICT_NAME: &str = "geolite_strict";
const TOKENIZER_LOWER_NAME: &str = "geolite_lower";

// exact and fuzzy matches carry separate boosts per field because tantivy scores a fuzzy match
// with the idf of the indexed term it landed on, not the query's: a common query token that
// fuzzy-matches a rare indexed term would inherit the rare term's idf and outrank real matches.
// an exact TermQuery in parallel with the larger boost keeps exact matches on top and lets the
// fuzzy ones compete only when nothing exact matched. the phrase boost fires only when the query
// tokens appear contiguous and in order, and is large enough to dominate the base bm25. strict
// fields keep case and diacritics, lower fields keep only diacritics; their boosts break ties
// when the document was indexed in the identical form.
#[derive(Clone, Copy)]
pub struct tantivy_boosts {
  pub name_exact: f32,
  pub name_fuzzy: f32,
  pub name_phrase: f32,
  pub name_strict: f32,
  pub name_lower: f32,
  pub hier_exact: f32,
  pub hier_fuzzy: f32,
  pub hier_phrase: f32,
  pub hier_strict: f32,
  pub hier_lower: f32,
}

// short tokens (post codes, abbreviations) collide easily, so they get one edit of tolerance;
// longer ones tolerate two without losing precision
fn fuzzy_distance_for(token: &str) -> u8 {
  if token.chars().count() < 4 { 1 } else { 2 }
}

const WRITER_MEMORY_BUDGET: usize = 50_000_000;

#[allow(clippy::type_complexity)]
fn schema() -> (Schema, Field, Field, Field, Field, Field, Field, Field, Field) {
  let mut builder = Schema::builder();
  // STORED to hand the id back, INDEXED to restrict a search to a set of ids (the region filter)
  let admin_level_id = builder.add_u64_field("admin_level_id", STORED | INDEXED);
  let admin_level = builder.add_u64_field("admin_level", INDEXED);
  let folded_indexing = TextFieldIndexing::default()
    .set_tokenizer(TOKENIZER_NAME)
    .set_index_option(IndexRecordOption::WithFreqsAndPositions);
  let folded_opts = TextOptions::default().set_indexing_options(folded_indexing);
  let name = builder.add_text_field("name", folded_opts.clone());
  let hier = builder.add_text_field("hier", folded_opts);
  let strict_indexing = TextFieldIndexing::default()
    .set_tokenizer(TOKENIZER_STRICT_NAME)
    .set_index_option(IndexRecordOption::WithFreqs);
  let strict_opts = TextOptions::default().set_indexing_options(strict_indexing);
  let name_strict = builder.add_text_field("name_strict", strict_opts.clone());
  let hier_strict = builder.add_text_field("hier_strict", strict_opts);
  let lower_indexing = TextFieldIndexing::default()
    .set_tokenizer(TOKENIZER_LOWER_NAME)
    .set_index_option(IndexRecordOption::WithFreqs);
  let lower_opts = TextOptions::default().set_indexing_options(lower_indexing);
  let name_lower = builder.add_text_field("name_lower", lower_opts.clone());
  let hier_lower = builder.add_text_field("hier_lower", lower_opts);
  (
    builder.build(),
    admin_level_id,
    admin_level,
    name,
    hier,
    name_strict,
    hier_strict,
    name_lower,
    hier_lower,
  )
}

pub(crate) fn build_entity_text(name: &str, post_code: Option<&str>) -> String {
  match post_code.map(str::trim).filter(|s| !s.is_empty()) {
    Some(pc) => {
      let digits: String = pc.chars().filter(|c| c.is_ascii_digit()).collect();
      if digits.is_empty() || digits == pc {
        format!("{name} {pc}")
      } else {
        format!("{name} {pc} {digits}")
      }
    }
    None => name.to_string(),
  }
}

fn register_tokenizers(index: &Index) {
  let folded = TextAnalyzer::builder(SimpleTokenizer::default())
    .filter(LowerCaser)
    .filter(AsciiFoldingFilter)
    .build();
  index.tokenizers().register(TOKENIZER_NAME, folded);
  let strict = TextAnalyzer::builder(SimpleTokenizer::default()).build();
  index.tokenizers().register(TOKENIZER_STRICT_NAME, strict);
  let lower = TextAnalyzer::builder(SimpleTokenizer::default())
    .filter(LowerCaser)
    .build();
  index.tokenizers().register(TOKENIZER_LOWER_NAME, lower);
}

pub struct tantivy_index {
  reader: IndexReader,
  id_field: Field,
  admin_level_field: Field,
  name_field: Field,
  hier_field: Field,
  name_strict_field: Field,
  hier_strict_field: Field,
  name_lower_field: Field,
  hier_lower_field: Field,
  boosts: tantivy_boosts,
}

pub fn default_path_for(sqlite_path: &str) -> Option<PathBuf> {
  if sqlite_path.is_empty() || sqlite_path == ":memory:" {
    return None;
  }
  let p = Path::new(sqlite_path);
  let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("database");
  let parent = p.parent().unwrap_or(Path::new(""));
  Some(parent.join(format!("{stem}.tantivy")))
}

pub fn destroy(index_path: &Path) {
  let _ = std::fs::remove_dir_all(index_path);
}

// expands abbreviations bidirectionally so query and indexed doc may use either
// form. e.g. with `("r.", "rua")`: a name containing "rua" gets "r." added; a
// name containing "r." gets "rua" added. variants are concatenated into one
// text so the tokenizer indexes both forms at the same position cost.
fn expand_abbreviations(text: &str, abbreviations: &[(&str, &str)]) -> String {
  if abbreviations.is_empty() {
    return text.to_string();
  }
  let lower = text.to_lowercase();
  let mut variants = vec![text.to_string()];
  for (abbrev, expansion) in abbreviations {
    if lower.contains(*expansion) {
      variants.push(lower.replace(*expansion, abbrev));
    } else if lower.contains(*abbrev) {
      variants.push(lower.replace(*abbrev, expansion));
    }
  }
  variants.join(" ")
}

pub struct progress_report {
  pub total: Option<u64>,
  pub processed: u64,
}

pub fn run(
  conn: &Connection,
  index_path: &Path,
  preset: &crate::presets::index_user_friendly_name_preset,
  progress: impl Fn(progress_report),
) -> tantivy_index {
  let total = repository::count(conn) as u64;
  progress(progress_report {
    total: Some(total),
    processed: 0,
  });
  let index = build(conn, index_path, preset.boosts, preset.abbreviations);
  progress(progress_report {
    total: Some(total),
    processed: total,
  });
  index
}

pub fn build(
  conn: &Connection,
  index_path: &Path,
  boosts: tantivy_boosts,
  abbreviations: &[(&str, &str)],
) -> tantivy_index {
  const SQL_LOAD_NAMES: &str = "
    SELECT id, name, post_code, admin_level
    FROM admin_levels
  ";

  const SQL_LOAD_HIERARCHY: &str = "
    SELECT admin_level_id, json(ancestor_ids)
    FROM admin_levels_hierarchy
  ";

  let _ = std::fs::remove_dir_all(index_path);
  std::fs::create_dir_all(index_path).expect("failed to create tantivy index dir");

  let (
    schema,
    id_field,
    admin_level_field,
    name_field,
    hier_field,
    name_strict_field,
    hier_strict_field,
    name_lower_field,
    hier_lower_field,
  ) = schema();
  let index = Index::create_in_dir(index_path, schema)
    .expect("failed to create tantivy index");
  register_tokenizers(&index);

  // the entity text carries the post code in its original form and digits-only, so that
  // "01310-100" and "01310100" both find the document: the simple tokenizer splits on the hyphen,
  // giving ["01310", "100"], while the digits-only form stays one token
  let mut names_map: HashMap<i64, String> = HashMap::new();
  let mut levels_map: HashMap<i64, u64> = HashMap::new();
  {
    let mut stmt = conn
      .prepare(SQL_LOAD_NAMES)
      .expect("failed to prepare load_names");
    let rows = stmt
      .query_map([], |row| {
        let id: i64 = row.get(0)?;
        let name: String = row.get(1)?;
        let post_code: Option<String> = row.get(2)?;
        let admin_level: u8 = row.get(3)?;
        Ok((id, build_entity_text(&name, post_code.as_deref()), admin_level))
      })
      .expect("failed to query admin_levels names");
    for row in rows.filter_map(|r| r.ok()) {
      let (id, entity_text, admin_level) = row;
      names_map.insert(id, entity_text);
      levels_map.insert(id, admin_level as u64);
    }
  }

  let mut writer = index
    .writer(WRITER_MEMORY_BUDGET)
    .expect("failed to create tantivy writer");

  let mut stmt = conn
    .prepare(SQL_LOAD_HIERARCHY)
    .expect("failed to prepare load_hierarchy");
  let rows = stmt
    .query_map([], |row| {
      Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })
    .expect("failed to query admin_levels_hierarchy");

  for row in rows {
    let (id, ancestors_json) = row.expect("failed to read hierarchy row");
    let ancestors = decode_chain(&ancestors_json);
    let own_name = names_map.get(&id).cloned().unwrap_or_default();
    let hier_text: String = ancestors
      .iter()
      .filter_map(|aid| names_map.get(aid).cloned())
      .collect::<Vec<_>>()
      .join(" ");
    let own_name_folded = expand_abbreviations(&own_name, abbreviations);
    let hier_text_folded = expand_abbreviations(&hier_text, abbreviations);
    let mut doc = TantivyDocument::default();
    doc.add_u64(id_field, id as u64);
    doc.add_u64(admin_level_field, levels_map.get(&id).copied().unwrap_or(0));
    doc.add_text(name_field, &own_name_folded);
    doc.add_text(hier_field, &hier_text_folded);
    doc.add_text(name_strict_field, &own_name);
    doc.add_text(hier_strict_field, &hier_text);
    doc.add_text(name_lower_field, &own_name);
    doc.add_text(hier_lower_field, &hier_text);
    writer
      .add_document(doc)
      .expect("failed to add tantivy document");
  }
  drop(stmt);
  writer.commit().expect("failed to commit tantivy writer");

  let reader = index
    .reader()
    .expect("failed to open tantivy reader");
  tantivy_index {
    reader,
    id_field,
    admin_level_field,
    name_field,
    hier_field,
    name_strict_field,
    hier_strict_field,
    name_lower_field,
    hier_lower_field,
    boosts,
  }
}

pub fn load(index_path: &Path, boosts: tantivy_boosts) -> Option<tantivy_index> {
  if !index_path.exists() {
    return None;
  }
  let index = Index::open_in_dir(index_path).ok()?;
  register_tokenizers(&index);
  let reader = index.reader().ok()?;
  let schema = index.schema();
  let id_field = schema.get_field("admin_level_id").ok()?;
  let admin_level_field = schema.get_field("admin_level").ok()?;
  let name_field = schema.get_field("name").ok()?;
  let hier_field = schema.get_field("hier").ok()?;
  let name_strict_field = schema.get_field("name_strict").ok()?;
  let hier_strict_field = schema.get_field("hier_strict").ok()?;
  let name_lower_field = schema.get_field("name_lower").ok()?;
  let hier_lower_field = schema.get_field("hier_lower").ok()?;
  Some(tantivy_index {
    reader,
    id_field,
    admin_level_field,
    name_field,
    hier_field,
    name_strict_field,
    hier_strict_field,
    name_lower_field,
    hier_lower_field,
    boosts,
  })
}

// the same pipeline the build runs (lower case, ascii folding), so query and document tokens
// agree; a one-shot analyzer, because reusing the index's would mean loading it from the registry
pub(crate) fn tokenize(text: &str) -> Vec<String> {
  let mut analyzer = TextAnalyzer::builder(SimpleTokenizer::default())
    .filter(LowerCaser)
    .filter(AsciiFoldingFilter)
    .build();
  let mut tokens = Vec::new();
  let mut stream = analyzer.token_stream(text);
  while let Some(t) = stream.next() {
    tokens.push(t.text.clone());
  }
  tokens
}

// no filter at all: the query tokens for the strict fields, which keep case and diacritics
fn tokenize_strict(text: &str) -> Vec<String> {
  let mut analyzer = TextAnalyzer::builder(SimpleTokenizer::default()).build();
  let mut tokens = Vec::new();
  let mut stream = analyzer.token_stream(text);
  while let Some(t) = stream.next() {
    tokens.push(t.text.clone());
  }
  tokens
}

// lower case only: the query tokens for the lower fields, which keep diacritics but not case
fn tokenize_lower(text: &str) -> Vec<String> {
  let mut analyzer = TextAnalyzer::builder(SimpleTokenizer::default())
    .filter(LowerCaser)
    .build();
  let mut tokens = Vec::new();
  let mut stream = analyzer.token_stream(text);
  while let Some(t) = stream.next() {
    tokens.push(t.text.clone());
  }
  tokens
}

type clause = (Occur, Box<dyn Query>);

fn boosted(query: impl Query + 'static, boost: f32) -> Box<dyn Query> {
  Box::new(BoostQuery::new(Box::new(query), boost))
}

// a Should clause per token against each field: it fires only when the document was indexed in
// the identical form, which breaks ties between "AAA" and "aaa", or "Praça" and "Praca"
fn bonus_clauses(tokens: &[String], fields: [(Field, f32); 2]) -> Vec<clause> {
  let mut clauses: Vec<clause> = Vec::with_capacity(tokens.len() * 2);
  for token in tokens {
    for (field, boost) in fields {
      let term = TermQuery::new(Term::from_field_text(field, token), IndexRecordOption::WithFreqs);
      clauses.push((Occur::Should, boosted(term, boost)));
    }
  }
  clauses
}

fn run_query(
  searcher: &Searcher,
  query: BooleanQuery,
  id_field: Field,
  limit: usize,
) -> Vec<(i64, f32)> {
  searcher
    .search(&query, &TopDocs::with_limit(limit).order_by_score())
    .unwrap_or_default()
    .into_iter()
    .filter_map(|(score, addr)| {
      let doc: TantivyDocument = searcher.doc(addr).ok()?;
      let id = doc.get_first(id_field).and_then(|v| v.as_u64())? as i64;
      Some((id, score))
    })
    .collect()
}

impl tantivy_index {
  // two queries with a fallback. the strict one demands every token exactly, in the name or in
  // the ancestry, and wins when it finds anything: only documents covering the whole query come
  // back. the loose one runs only when the strict one is empty: exact and fuzzy terms as Should
  // clauses, which covers a typo, an extra word or partial coverage. the score is the raw bm25
  // of whichever query found the document, in the order tantivy delivered.
  pub fn search(
    &self,
    query: &str,
    limit: usize,
    last_admin_levels: Option<&[level]>,
    allowed_ids: Option<&[i64]>,
  ) -> Vec<(i64, f32)> {
    let tokens = tokenize(query);
    if tokens.is_empty() {
      return vec![];
    }
    let searcher = self.reader.searcher();

    let mut strict = self.strict_clauses(query, &tokens);
    strict.extend(self.filter_clauses(last_admin_levels, allowed_ids));
    let strict_hits = run_query(&searcher, BooleanQuery::new(strict), self.id_field, limit);
    if !strict_hits.is_empty() {
      return strict_hits;
    }

    let mut loose = self.loose_clauses(&tokens);
    loose.extend(self.filter_clauses(last_admin_levels, allowed_ids));
    run_query(&searcher, BooleanQuery::new(loose), self.id_field, limit)
  }

  fn strict_clauses(&self, query: &str, tokens: &[String]) -> Vec<clause> {
    let mut clauses: Vec<clause> = tokens
      .iter()
      .map(|token| {
        let exact_name = boosted(
          TermQuery::new(Term::from_field_text(self.name_field, token), IndexRecordOption::WithFreqs),
          self.boosts.name_exact,
        );
        let exact_hier = boosted(
          TermQuery::new(Term::from_field_text(self.hier_field, token), IndexRecordOption::WithFreqs),
          self.boosts.hier_exact,
        );
        let token_query = BooleanQuery::new(vec![
          (Occur::Should, exact_name),
          (Occur::Should, exact_hier),
        ]);
        (Occur::Must, Box::new(token_query) as Box<dyn Query>)
      })
      .collect();

    // a phrase bonus when the tokens appear contiguous and in order, which tells apart documents
    // sharing the same terms in a different order; meaningless for a single token
    if tokens.len() >= 2 {
      for (field, boost) in [
        (self.name_field, self.boosts.name_phrase),
        (self.hier_field, self.boosts.hier_phrase),
      ] {
        let terms: Vec<Term> = tokens
          .iter()
          .map(|t| Term::from_field_text(field, t))
          .collect();
        clauses.push((Occur::Should, boosted(PhraseQuery::new(terms), boost)));
      }
    }

    clauses.extend(bonus_clauses(
      &tokenize_strict(query),
      [
        (self.name_strict_field, self.boosts.name_strict),
        (self.hier_strict_field, self.boosts.hier_strict),
      ],
    ));
    clauses.extend(bonus_clauses(
      &tokenize_lower(query),
      [
        (self.name_lower_field, self.boosts.name_lower),
        (self.hier_lower_field, self.boosts.hier_lower),
      ],
    ));
    clauses
  }

  fn loose_clauses(&self, tokens: &[String]) -> Vec<clause> {
    let mut clauses: Vec<clause> = Vec::with_capacity(tokens.len() * 4);
    for token in tokens {
      let distance = fuzzy_distance_for(token);
      for (field, exact_boost, fuzzy_boost) in [
        (self.name_field, self.boosts.name_exact, self.boosts.name_fuzzy),
        (self.hier_field, self.boosts.hier_exact, self.boosts.hier_fuzzy),
      ] {
        let term = Term::from_field_text(field, token);
        clauses.push((
          Occur::Should,
          boosted(TermQuery::new(term.clone(), IndexRecordOption::WithFreqs), exact_boost),
        ));
        clauses.push((
          Occur::Should,
          boosted(FuzzyTermQuery::new(term, distance, true), fuzzy_boost),
        ));
      }
    }
    clauses
  }

  // both filters are Must clauses with boost 0.0: they restrict the document set without touching
  // the score, so the bm25 ranking stays purely textual
  fn filter_clauses(
    &self,
    last_admin_levels: Option<&[level]>,
    allowed_ids: Option<&[i64]>,
  ) -> Vec<clause> {
    let mut clauses: Vec<clause> = Vec::new();
    if let Some(levels) = last_admin_levels {
      let shoulds: Vec<clause> = levels
        .iter()
        .map(|level| {
          let term = Term::from_field_u64(self.admin_level_field, level.value() as u64);
          (
            Occur::Should,
            Box::new(TermQuery::new(term, IndexRecordOption::Basic)) as Box<dyn Query>,
          )
        })
        .collect();
      clauses.push((Occur::Must, boosted(BooleanQuery::new(shoulds), 0.0)));
    }
    if let Some(ids) = allowed_ids {
      let terms = ids
        .iter()
        .map(|&id| Term::from_field_u64(self.id_field, id as u64));
      clauses.push((Occur::Must, boosted(TermSetQuery::new(terms), 0.0)));
    }
    clauses
  }
}

#[cfg(test)]
pub(crate) mod testing {
  use super::tantivy_index;
  use rusqlite::Connection;
  use std::path::PathBuf;

  pub struct tempdir_guard {
    pub path: PathBuf,
  }

  impl tempdir_guard {
    pub fn new() -> Self {
      static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
      let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
      let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
      let path = std::env::temp_dir().join(format!(
        "geolite-test-tantivy-{}-{nanos}-{seq}",
        std::process::id()
      ));
      Self { path }
    }
  }

  impl Drop for tempdir_guard {
    fn drop(&mut self) {
      let _ = std::fs::remove_dir_all(&self.path);
    }
  }

  pub fn build_test_index(conn: &Connection) -> (tempdir_guard, tantivy_index) {
    let guard = tempdir_guard::new();
    let index = super::build(
      conn,
      &guard.path,
      crate::presets::resolve(None).index_user_friendly_name.boosts,
      &[],
    );
    (guard, index)
  }
}

#[cfg(test)]
#[path = "search_index.test.rs"]
mod tests;
