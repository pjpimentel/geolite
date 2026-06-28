use rusqlite::Connection;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tantivy::{
  Index, IndexReader, TantivyDocument,
  collector::TopDocs,
  query::{
    BooleanQuery, BoostQuery, FuzzyTermQuery, Occur, PhraseQuery, Query, TermQuery, TermSetQuery,
  },
  schema::{
    Field, INDEXED, IndexRecordOption, STORED, Schema, TextFieldIndexing, TextOptions, Value,
  },
  tokenizer::{AsciiFoldingFilter, LowerCaser, SimpleTokenizer, TextAnalyzer},
};

const SQL_LOAD_NAMES: &str = "
  SELECT id, name, post_code, admin_level
  FROM admin_levels
";

const SQL_LOAD_HIERARCHY: &str = "
  SELECT admin_level_id, json(ancestor_ids)
  FROM admin_levels_hierarchy
";

const TOKENIZER_NAME: &str = "geolite_ascii";
const TOKENIZER_STRICT_NAME: &str = "geolite_strict";
const TOKENIZER_LOWER_NAME: &str = "geolite_lower";

// boosts separados pra match exato vs fuzzy, em cada campo.
// motivacao: tantivy/lucene `FuzzyTermQuery` pontua usando o IDF do termo do
// INDICE que casou (nao o da query). entao query "praca" (comum) que fuzzy-casa
// "branca" (rara) ganharia score inflado pelo IDF de "branca" mesmo sendo
// match ruim. solução: adicionar TermQuery exato em paralelo ao fuzzy com boost
// maior — matches exatos dominam, fuzzy só compete quando nada exato casa.
// PhraseQuery dispara só quando os tokens da query aparecem contíguos e na
// mesma ordem no campo. boost grande pra dominar BM25 base.
// strict fields preservam case e diacrítico; lower fields preservam diacrítico
// mas não case. boosts discriminam quando o doc tem forma idêntica indexada.
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

// edit distance adaptativo por tamanho do token: tokens curtos (CEPs, abrev)
// exigem precisao (d=1) porque colidem facil (ex.: "100" vs "001" é d=2 mas
// CEPs diferentes); tokens longos toleram typos (d=2) sem perder precisão.
fn fuzzy_distance_for(token: &str) -> u8 {
  if token.chars().count() < 4 { 1 } else { 2 }
}

const WRITER_MEMORY_BUDGET: usize = 50_000_000;

#[allow(clippy::type_complexity)]
fn schema() -> (Schema, Field, Field, Field, Field, Field, Field, Field, Field) {
  let mut builder = Schema::builder();
  // STORED para retornar o id; INDEXED para restringir a busca a um conjunto de ids (filtro espacial)
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

pub fn build(
  conn: &Connection,
  index_path: &Path,
  boosts: tantivy_boosts,
  abbreviations: &[(&str, &str)],
) -> tantivy_index {
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

  // entity_text inclui name + postcode (formato original + digits-only) pra que
  // queries como "01310-100" E "01310100" casem o mesmo doc. com asciifolding +
  // simple tokenizer, o hifen vira separador entao "01310-100" tokeniza em
  // ["01310","100"] e a versao digits-only "01310100" entra como token unico.
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
    let ancestors: Vec<i64> = serde_json::from_str(&ancestors_json).unwrap_or_default();
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

// passa pelo mesmo pipeline de tokens que o build pra garantir consistencia
// (asciifolding, lowercase). usa um analyzer one-shot — reaproveitar do index
// exigiria carregar do registry, complica e o custo de criar e marginal.
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

// tokenize sem nenhum filtro — usado pra gerar tokens da query que vão bater
// contra os campos strict (que preservam case e diacrítico).
fn tokenize_strict(text: &str) -> Vec<String> {
  let mut analyzer = TextAnalyzer::builder(SimpleTokenizer::default()).build();
  let mut tokens = Vec::new();
  let mut stream = analyzer.token_stream(text);
  while let Some(t) = stream.next() {
    tokens.push(t.text.clone());
  }
  tokens
}

// tokenize só com LowerCaser — usado pra gerar tokens da query que vão bater
// contra os campos lower (preservam diacrítico, case-insensitive).
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

impl tantivy_index {
  // estratégia em duas queries com fallback:
  //   Q1 strict — "input completo": todo token tem que casar EXATO em
  //     name OU hier (Occur::Must por token, com sub-Should entre os dois
  //     campos). garante precisão: só docs que cobrem 100% da intenção
  //     exata retornam. se Q1 retorna algo, usa esse resultado e PÁRA.
  //   Q2 loose — só roda se Q1 vier vazia. termos splitados em Should com
  //     exact + fuzzy nos dois campos (comportamento legado). cobre
  //     queries com typo, palavra extra, ou cobertura parcial.
  // score retornado é o BM25 cru da query que efetivamente achou o doc;
  // ordem é a que tantivy entregou (sem reordenação aqui no client).
  pub fn search(
    &self,
    query: &str,
    limit: usize,
    last_admin_levels: Option<&[u8]>,
    allowed_ids: Option<&[i64]>,
  ) -> Vec<(i64, f32)> {
    let tokens = tokenize(query);
    if tokens.is_empty() {
      return vec![];
    }
    let strict_tokens = tokenize_strict(query);
    let lower_tokens = tokenize_lower(query);
    let searcher = self.reader.searcher();

    // filtro por nivel como Must com boost 0.0: restringe o conjunto de docs sem
    // contribuir pro score, entao o ranking BM25 continua puramente textual
    let level_filter_clause = || -> Option<(Occur, Box<dyn Query>)> {
      last_admin_levels.map(|levels| {
        let shoulds: Vec<(Occur, Box<dyn Query>)> = levels
          .iter()
          .map(|level| {
            let term = tantivy::Term::from_field_u64(self.admin_level_field, *level as u64);
            (
              Occur::Should,
              Box::new(TermQuery::new(term, IndexRecordOption::Basic)) as Box<dyn Query>,
            )
          })
          .collect();
        (
          Occur::Must,
          Box::new(BoostQuery::new(Box::new(BooleanQuery::new(shoulds)), 0.0)) as Box<dyn Query>,
        )
      })
    };

    // filtro espacial: restringe o ranking a um conjunto de ids (regiao do envelope), via
    // TermSetQuery sobre admin_level_id (Must, boost 0.0 — filtra sem contribuir pro score)
    let id_filter_clause = || -> Option<(Occur, Box<dyn Query>)> {
      allowed_ids.map(|ids| {
        let terms = ids
          .iter()
          .map(|&id| tantivy::Term::from_field_u64(self.id_field, id as u64));
        (
          Occur::Must,
          Box::new(BoostQuery::new(Box::new(TermSetQuery::new(terms)), 0.0)) as Box<dyn Query>,
        )
      })
    };

    let run = |query: BooleanQuery| -> Vec<(i64, f32)> {
      searcher
        .search(&query, &TopDocs::with_limit(limit).order_by_score())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(score, addr)| {
          let doc: TantivyDocument = searcher.doc(addr).ok()?;
          let id = doc
            .get_first(self.id_field)
            .and_then(|v| v.as_u64())? as i64;
          Some((id, score))
        })
        .collect()
    };

    let mut strict_clauses: Vec<(Occur, Box<dyn Query>)> = tokens
      .iter()
      .map(|token| {
        let term_name = tantivy::Term::from_field_text(self.name_field, token);
        let term_hier = tantivy::Term::from_field_text(self.hier_field, token);
        let exact_name = BoostQuery::new(
          Box::new(TermQuery::new(term_name, IndexRecordOption::WithFreqs)),
          self.boosts.name_exact,
        );
        let exact_hier = BoostQuery::new(
          Box::new(TermQuery::new(term_hier, IndexRecordOption::WithFreqs)),
          self.boosts.hier_exact,
        );
        let token_query = BooleanQuery::new(vec![
          (Occur::Should, Box::new(exact_name) as Box<dyn Query>),
          (Occur::Should, Box::new(exact_hier) as Box<dyn Query>),
        ]);
        (Occur::Must, Box::new(token_query) as Box<dyn Query>)
      })
      .collect();

    // bônus por frase: quando os tokens da query aparecem na mesma ordem em
    // name (ou hier), o doc ganha boost. discrimina entre docs que têm os
    // mesmos termos mas em ordens diferentes. só faz sentido com 2+ tokens.
    if tokens.len() >= 2 {
      for (field, boost) in [
        (self.name_field, self.boosts.name_phrase),
        (self.hier_field, self.boosts.hier_phrase),
      ] {
        let terms: Vec<tantivy::Term> = tokens
          .iter()
          .map(|t| tantivy::Term::from_field_text(field, t))
          .collect();
        let phrase = PhraseQuery::new(terms);
        strict_clauses.push((
          Occur::Should,
          Box::new(BoostQuery::new(Box::new(phrase), boost)),
        ));
      }
    }

    // bônus strict: para cada token da query na forma original (sem lower nem
    // fold), Should clause contra os campos *_strict. dispara só quando o doc
    // tem a forma idêntica indexada, discriminando p.ex. "AAA" vs "aaa".
    for token in &strict_tokens {
      for (field, boost) in [
        (self.name_strict_field, self.boosts.name_strict),
        (self.hier_strict_field, self.boosts.hier_strict),
      ] {
        let term = tantivy::Term::from_field_text(field, token);
        let strict = TermQuery::new(term, IndexRecordOption::WithFreqs);
        strict_clauses.push((
          Occur::Should,
          Box::new(BoostQuery::new(Box::new(strict), boost)),
        ));
      }
    }

    // bônus lower: tokens da query em lowercase mas com diacrítico preservado,
    // Should clause contra os campos *_lower. discrimina "Praça"/"Praca"
    // independente de case da query. resolve o desempate entre docs que
    // colidem no campo folded mas têm acentos diferentes.
    for token in &lower_tokens {
      for (field, boost) in [
        (self.name_lower_field, self.boosts.name_lower),
        (self.hier_lower_field, self.boosts.hier_lower),
      ] {
        let term = tantivy::Term::from_field_text(field, token);
        let lower = TermQuery::new(term, IndexRecordOption::WithFreqs);
        strict_clauses.push((
          Occur::Should,
          Box::new(BoostQuery::new(Box::new(lower), boost)),
        ));
      }
    }

    if let Some(clause) = level_filter_clause() {
      strict_clauses.push(clause);
    }
    if let Some(clause) = id_filter_clause() {
      strict_clauses.push(clause);
    }

    let strict_hits = run(BooleanQuery::new(strict_clauses));
    if !strict_hits.is_empty() {
      return strict_hits;
    }

    let mut loose_clauses: Vec<(Occur, Box<dyn Query>)> = Vec::with_capacity(tokens.len() * 4);
    for token in &tokens {
      let distance = fuzzy_distance_for(token);
      for (field, exact_boost, fuzzy_boost) in [
        (self.name_field, self.boosts.name_exact, self.boosts.name_fuzzy),
        (self.hier_field, self.boosts.hier_exact, self.boosts.hier_fuzzy),
      ] {
        let term = tantivy::Term::from_field_text(field, token);
        let exact = TermQuery::new(term.clone(), IndexRecordOption::WithFreqs);
        loose_clauses.push((
          Occur::Should,
          Box::new(BoostQuery::new(Box::new(exact), exact_boost)),
        ));
        let fuzzy = FuzzyTermQuery::new(term, distance, true);
        loose_clauses.push((
          Occur::Should,
          Box::new(BoostQuery::new(Box::new(fuzzy), fuzzy_boost)),
        ));
      }
    }
    if let Some(clause) = level_filter_clause() {
      loose_clauses.push(clause);
    }
    if let Some(clause) = id_filter_clause() {
      loose_clauses.push(clause);
    }
    run(BooleanQuery::new(loose_clauses))
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
