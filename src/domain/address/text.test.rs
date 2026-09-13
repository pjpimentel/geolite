use crate::domain::address::{address, house_number_match, query_opts, query_output};
use crate::domain::admin_level::repository::batch_upsert;
use crate::domain::admin_level::{admin_level as admin_levels_row, level};
use crate::domain::admin_level_hierarchy::search_index::tantivy_index;
use crate::domain::admin_level_hierarchy::search_index::testing::{build_test_index, tempdir_guard};
use crate::domain::house_number::house_number_policy;
use geo::{Coord, Geometry, LineString};
use rusqlite::Connection;

const POLICY: house_number_policy = crate::presets::DEFAULT.house_numbers;

const THREE_NUMBERS: [(&str, f64, f64); 3] = [
  ("100", -46.31980, -23.97240),
  ("200", -46.31960, -23.97220),
  ("300", -46.31940, -23.97200),
];

fn make_street_row(name: &str, lon_offset: f64, way_id: u64) -> admin_levels_row {
  make_street_row_with_postcode(name, lon_offset, way_id, None)
}

fn make_street_row_with_postcode(
  name: &str,
  lon_offset: f64,
  way_id: u64,
  post_code: Option<&str>,
) -> admin_levels_row {
  let ls = LineString(vec![
    Coord {
      x: -46.31980 + lon_offset,
      y: -23.97241,
    },
    Coord {
      x: -46.31979 + lon_offset,
      y: -23.97240,
    },
  ]);
  admin_levels_row {
    relation_id: None,
    way_id: Some(way_id),
    level: level::street,
    wkb: Geometry::LineString(ls).into(),
    name: name.to_string(),
    country_iso_code: None,
    post_code: post_code.map(str::to_string),
  }
}

// the areas stored, boxed in the rtree, chained and indexed: what every text scenario asks
struct scene {
  conn: Connection,
  _index_dir: tempdir_guard,
  index: tantivy_index,
}

impl scene {
  fn of(rows: &[admin_levels_row]) -> scene {
    let conn = crate::database::open_write(":memory:");
    batch_upsert(&conn, rows);
    crate::domain::admin_level::spatial_index::run(&conn, |_| {});
    crate::domain::admin_level_hierarchy::resolver::run(&conn, |_| {});
    let (index_dir, index) = build_test_index(&conn);
    scene {
      conn,
      _index_dir: index_dir,
      index,
    }
  }

  // one street carrying the given numbers, node ids counted from 1
  fn numbered_street(name: &str, numbers: &[(&str, f64, f64)]) -> scene {
    let s = scene::of(&[make_street_row(name, 0.000, 1)]);
    let id = s.street_id(name);
    for (i, (number, lon, lat)) in numbers.iter().enumerate() {
      s.insert_house_number(id, i as u64 + 1, number, *lon, *lat);
    }
    s
  }

  fn ask(&self, text: &str) -> query_output {
    self.ask_with(
      text,
      &query_opts {
        include_wkt: true,
        ..Default::default()
      },
    )
  }

  fn ask_with(&self, text: &str, opts: &query_opts) -> query_output {
    address::open(&self.conn, Some(&self.index), &POLICY).query_by_text(text, opts)
  }

  fn street_id(&self, name: &str) -> i64 {
    self
      .conn
      .query_row("SELECT id FROM admin_levels WHERE name = ?1", [name], |r| {
        r.get(0)
      })
      .expect("failed to read street id")
  }

  fn id_by_way(&self, way_id: u64) -> i64 {
    self
      .conn
      .query_row(
        "SELECT id FROM admin_levels WHERE way_id = ?1",
        [way_id],
        |r| r.get(0),
      )
      .expect("failed to read id by way")
  }

  fn insert_house_number(&self, admin_level_id: i64, node_id: u64, number: &str, lon: f64, lat: f64) {
    crate::domain::house_number::repository::batch_insert_links(
      &self.conn,
      &[crate::domain::house_number::fixtures::link(
        node_id,
        admin_level_id,
        number,
        lon,
        lat,
      )],
    );
  }
}

fn leaf_names(out: &query_output) -> Vec<String> {
  out
    .matches
    .iter()
    .filter_map(|m| m.admin_levels.last().map(|al| al.name.clone()))
    .collect()
}

fn street_name(out: &query_output) -> Option<String> {
  out
    .matches
    .first()
    .and_then(|m| m.admin_levels.iter().find(|a| a.level == 12))
    .map(|a| a.name.clone())
}

fn with_levels(levels: Vec<level>) -> query_opts<'static> {
  query_opts {
    last_admin_levels: Some(levels),
    include_wkt: true,
    ..Default::default()
  }
}

#[test]
fn _00_text_search_prioritizes_exact_case_match() {
  let s = scene::of(&[
    make_street_row("aaa", 0.000, 1),
    make_street_row("AAA", 0.001, 2),
    make_street_row("BBB", 0.002, 3),
    make_street_row("bbb", 0.003, 4),
  ]);

  assert_eq!(leaf_names(&s.ask("AAA")), vec!["AAA", "aaa"]);
  assert_eq!(leaf_names(&s.ask("bbb")), vec!["bbb", "BBB"]);
}

#[test]
fn _01_text_search_prioritizes_closest_spelling_variant() {
  let s = scene::of(&[
    make_street_row("Brasil", 0.000, 1),
    make_street_row("Brazil", 0.001, 2),
  ]);

  assert_eq!(leaf_names(&s.ask("brasil")), vec!["Brasil"]);
  assert_eq!(leaf_names(&s.ask("brazil")), vec!["Brazil"]);
}

#[test]
fn _02_text_search_prioritizes_matching_diacritic_variant() {
  let s = scene::of(&[
    make_street_row("Praça", 0.000, 1),
    make_street_row("Praca", 0.001, 2),
  ]);

  assert_eq!(leaf_names(&s.ask("praça")), vec!["Praça", "Praca"]);
  assert_eq!(leaf_names(&s.ask("Praca")), vec!["Praca", "Praça"]);
}

#[test]
fn _03_text_search_prioritizes_record_with_matching_hierarchy() {
  let s = scene::of(&[
    make_street_row("rua castro alves", 0.000, 1),
    make_street_row("rua castro alves, embare", 0.001, 2),
    make_street_row("rua castro alves, embare, santos", 0.002, 3),
  ]);

  let first_match = |input: &str| -> Option<String> {
    s.ask(input).matches.first().map(|m| m.friendly_name.clone())
  };

  assert_eq!(
    first_match("rua castro alves").as_deref(),
    Some("rua castro alves"),
  );
  assert_eq!(
    first_match("rua castro alves embare").as_deref(),
    Some("rua castro alves, embare"),
  );
  assert_eq!(
    first_match("rua castro alves santos").as_deref(),
    Some("rua castro alves, embare, santos"),
  );
  assert_eq!(
    first_match("rua castro alves embare santos").as_deref(),
    Some("rua castro alves, embare, santos"),
  );
}

#[test]
fn _04_text_search_prioritizes_matching_word_order() {
  let s = scene::of(&[
    make_street_row("embare, rua castro alves", 0.000, 1),
    make_street_row("alves castro rua embare", 0.001, 2),
    make_street_row("rua castro alves embare", 0.002, 3),
  ]);

  let first = |input: &str| -> Option<String> {
    s.ask(input).matches.first().map(|m| m.friendly_name.clone())
  };
  assert_eq!(
    first("rua castro alves, embare").as_deref(),
    Some("rua castro alves embare")
  );
  assert_eq!(
    first("embare, rua castro alves").as_deref(),
    Some("embare, rua castro alves")
  );
}

#[test]
fn _05_text_search_finds_street_by_postcode_with_hyphen() {
  let s = scene::of(&[
    make_street_row_with_postcode("av paulista", 0.000, 1, Some("01310-100")),
    make_street_row_with_postcode("rua oscar freire", 0.001, 2, Some("01426-001")),
  ]);

  let names = leaf_names(&s.ask("01310-100"));
  assert!(names.contains(&"av paulista".to_string()));
  assert!(!names.contains(&"rua oscar freire".to_string()));
}

#[test]
fn _06_text_search_finds_street_by_postcode_without_hyphen() {
  let s = scene::of(&[
    make_street_row_with_postcode("av paulista", 0.000, 1, Some("01310-100")),
    make_street_row_with_postcode("rua oscar freire", 0.001, 2, Some("01426-001")),
  ]);

  let names = leaf_names(&s.ask("01310100"));
  assert!(names.contains(&"av paulista".to_string()));
}

#[test]
fn _07_friendly_name_contains_postcode_when_street_has_one() {
  let s = scene::of(&[make_street_row_with_postcode(
    "av paulista",
    0.000,
    1,
    Some("01310-100"),
  )]);

  let out = s.ask("av paulista");
  let first = out.matches.first().expect("expected one match");
  assert_eq!(first.friendly_name, "av paulista, 01310-100");
}

#[test]
fn _08_friendly_name_omits_postcode_when_street_has_none() {
  let s = scene::of(&[make_street_row("av paulista", 0.000, 1)]);

  let out = s.ask("av paulista");
  let first = out.matches.first().expect("expected one match");
  assert_eq!(first.friendly_name, "av paulista");
}

#[test]
fn _09_text_search_finds_match_when_input_has_extra_leading_word() {
  let s = scene::of(&[make_street_row("sehrs", 0.000, 1)]);
  assert_eq!(s.ask("hotel sehrs").matches.len(), 1);
}

#[test]
fn _10_text_search_finds_match_when_input_has_extra_trailing_word() {
  let s = scene::of(&[make_street_row("rua oscar freire", 0.000, 1)]);
  assert_eq!(s.ask("rua oscar freire ipanema").matches.len(), 1);
}

#[test]
fn _11_text_search_rank_results_as_expected() {
  let s = scene::of(&[
    make_street_row("sehrs", 0.000, 1),
    make_street_row("h seh", 0.002, 2),
    make_street_row("hot hrs", 0.003, 3),
    make_street_row("hot sehrs", 0.004, 4),
  ]);

  assert_eq!(
    leaf_names(&s.ask("hotel sehrs")),
    vec!["hot sehrs", "sehrs", "hot hrs", "h seh"]
  );
}

#[test]
fn _12_similarity_reflects_match_quality() {
  let s = scene::of(&[
    make_street_row_with_postcode("rua castro alves", 0.000, 1, Some("11250-000")),
    make_street_row("Praça da Sé", 0.002, 3),
  ]);

  let outputs = [
    s.ask("rua castro alves"),
    s.ask("rua"),
    s.ask("castro"),
    s.ask("alves"),
    s.ask("rua castro"),
    s.ask("castro alves"),
    s.ask("rua alves"),
    s.ask("alves castro rua"),
    s.ask("rua castro alvez"),
    s.ask("11250-000"),
    s.ask("hotel rua castro alves"),
    s.ask("praca da se"),
    s.ask("11250000"),
  ];

  assert_eq!(outputs[0].matches[0].similarity, Some(1.0));
  assert_eq!(outputs[1].matches[0].similarity, Some(1.0));
  assert_eq!(outputs[2].matches[0].similarity, Some(1.0));
  assert_eq!(outputs[3].matches[0].similarity, Some(1.0));
  assert_eq!(outputs[4].matches[0].similarity, Some(1.0));
  assert_eq!(outputs[5].matches[0].similarity, Some(1.0));
  assert_eq!(outputs[6].matches[0].similarity, Some(1.0));
  assert_eq!(outputs[7].matches[0].similarity, Some(1.0));
  assert_eq!(outputs[8].matches[0].similarity, Some(0.66667));
  assert_eq!(outputs[9].matches[0].similarity, Some(1.0));
  assert_eq!(outputs[10].matches[0].similarity, Some(0.75));
  assert_eq!(outputs[11].matches[0].similarity, Some(1.0));
  assert_eq!(outputs[12].matches[0].similarity, Some(1.0));

  assert_eq!(outputs[0].matches[0].score, Some(36.598167));
  assert_eq!(outputs[1].matches[0].score, Some(6.099695));
  assert_eq!(outputs[2].matches[0].score, Some(6.099695));
  assert_eq!(outputs[3].matches[0].score, Some(6.099695));
  assert_eq!(outputs[4].matches[0].score, Some(24.39878));
  assert_eq!(outputs[5].matches[0].score, Some(24.39878));
  assert_eq!(outputs[6].matches[0].score, Some(12.19939));
  assert_eq!(outputs[7].matches[0].score, Some(18.299086));
  assert_eq!(outputs[8].matches[0].score, Some(10.599695));
  assert_eq!(outputs[9].matches[0].score, Some(24.39878));
  assert_eq!(outputs[10].matches[0].score, Some(13.649543));
  assert_eq!(outputs[11].matches[0].score, Some(40.12957));
  assert_eq!(outputs[12].matches[0].score, Some(6.099695));
}

#[test]
fn _13_text_search_praca_doutor_hipolito_do_rego_embare_santos() {
  let s = scene::of(&[
    make_street_row("Praça Doutor Hipólito do Rego", 0.000, 1),
    make_street_row("Praça Santo Antônio do Embaré", 0.001, 2),
    make_street_row("Santo Antônio do Embaré", 0.002, 3),
    make_street_row("Rua Manoel Hipolito do Rego", 0.003, 4),
    make_street_row("Praça Hipólito Fernandes", 0.004, 5),
    make_street_row("Embaré", 0.005, 6),
  ]);

  assert_eq!(
    leaf_names(&s.ask("praca doutor hipolito do rego embare santos")),
    vec![
      "Praça Doutor Hipólito do Rego",
      "Praça Santo Antônio do Embaré",
      "Rua Manoel Hipolito do Rego",
      "Praça Hipólito Fernandes",
      "Santo Antônio do Embaré",
      "Embaré",
    ],
  );
}

#[test]
fn _14_text_search_with_trailing_house_number_resolves_exact_point() {
  let s = scene::numbered_street("rua oscar freire", &THREE_NUMBERS);

  let out = s.ask("rua oscar freire 100");
  let m = out.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert!(matches!(hn.kind, house_number_match::exact));
  assert_eq!(hn.number, "100");
  assert_eq!(m.latitude, -23.97240);
  assert_eq!(m.longitude, -46.31980);
  let last = m.admin_levels.last().expect("expected level");
  assert_eq!(last.level, 30);
  assert_eq!(last.name, "100");
  assert_eq!(m.friendly_name, "rua oscar freire, 100");
}

#[test]
fn _15_text_search_with_leading_house_number_resolves_exact_point() {
  let s = scene::numbered_street("rua oscar freire", &THREE_NUMBERS);

  let out = s.ask("100 rua oscar freire");
  let m = out.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert!(matches!(hn.kind, house_number_match::exact));
  assert_eq!(m.latitude, -23.97240);
  assert_eq!(m.longitude, -46.31980);
}

#[test]
fn _16_text_search_with_missing_house_number_interpolates_between_neighbors() {
  let s = scene::numbered_street("rua oscar freire", &THREE_NUMBERS);

  let out = s.ask("rua oscar freire 150");
  let m = out.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert!(matches!(hn.kind, house_number_match::interpolated));
  assert_eq!(hn.number, "150");
  assert_eq!(m.latitude, -23.97230);
  assert_eq!(m.longitude, -46.31970);
  assert_eq!(m.friendly_name, "rua oscar freire, 150");
}

#[test]
fn _17_text_search_with_unbracketable_house_number_marks_absent() {
  let s = scene::numbered_street("rua oscar freire", &THREE_NUMBERS);

  let out = s.ask("rua oscar freire 400");
  let m = out.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert!(matches!(hn.kind, house_number_match::absent));
  let last = m.admin_levels.last().expect("expected level");
  assert_eq!(last.level, 12);
  assert_eq!(m.friendly_name, "rua oscar freire");
}

#[test]
fn _18_text_search_by_postcode_does_not_strip_a_house_number() {
  let s = scene::of(&[
    make_street_row_with_postcode("av paulista", 0.000, 1, Some("01310-100")),
    make_street_row_with_postcode("rua oscar freire", 0.001, 2, Some("01426-001")),
  ]);

  for q in ["01310-100", "01310100"] {
    let out = s.ask(q);
    let m = out.matches.first().expect("expected one match");
    assert!(m.house_number.is_none());
    assert_eq!(
      m.admin_levels.last().map(|a| a.name.as_str()),
      Some("av paulista")
    );
  }
}

#[test]
fn _19_text_search_ignores_a_street_name_number_and_resolves_the_house_number() {
  let s = scene::numbered_street("rua 25 de marco", &[("100", -46.31980, -23.97240)]);

  let out = s.ask("rua 25 de marco 100");
  let m = out.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert_eq!(hn.number, "100");
  assert!(matches!(hn.kind, house_number_match::exact));
  assert_eq!(street_name(&out).as_deref(), Some("rua 25 de marco"));
}

// the house number is resolved the same way whether it comes right after the street name
// (brazilian format "rua x 35, bairro, cidade") or at the very end of the query.
#[test]
fn _20_house_number_resolves_the_same_in_the_middle_or_at_the_end() {
  let s = scene::numbered_street(
    "rua castro alves, embare, santos",
    &[("35", -46.31980, -23.97240)],
  );

  for q in [
    "rua castro alves, embare, santos 35",
    "rua castro alves 35, embare, santos",
  ] {
    let out = s.ask(q);
    let m = out.matches.first().expect("expected one match");
    let hn = m.house_number.as_ref().expect("expected house_number");
    assert!(matches!(hn.kind, house_number_match::exact));
    assert_eq!(hn.number, "35");
    assert_eq!(m.latitude, -23.97240);
    assert_eq!(m.longitude, -46.31980);
  }
}

// only the first numeric occurrence after the street name is taken as the house number — not
// whichever number happens to exist on the street. here "50" comes first, so the existing
// "200" is ignored and the result is absent (50 can't be placed).
#[test]
fn _21_uses_only_the_first_numeric_occurrence_after_the_street_name() {
  let s = scene::numbered_street("rua oscar freire", &[("200", -46.31960, -23.97220)]);

  let out = s.ask("rua oscar freire 50 200");
  let m = out.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert_eq!(hn.number, "50");
  assert!(matches!(hn.kind, house_number_match::absent));
}

// a street whose own name holds several numbers ("25" and "2024"): all of them are removed
// before picking the house number, so only a real trailing number resolves, and a query that
// is just the street name (no house number) resolves to no house_number at all.
#[test]
fn _22_ignores_every_number_that_belongs_to_the_street_name() {
  let s = scene::numbered_street("rua 25 de marco de 2024", &[("100", -46.31980, -23.97240)]);

  let with_number = s.ask("rua 25 de marco de 2024 100");
  let m = with_number.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert!(matches!(hn.kind, house_number_match::exact));
  assert_eq!(hn.number, "100");
  assert_eq!(
    street_name(&with_number).as_deref(),
    Some("rua 25 de marco de 2024")
  );

  let without_number = s.ask("rua 25 de marco de 2024");
  let m = without_number.matches.first().expect("expected one match");
  assert!(m.house_number.is_none());
}

// the same street is split into two osm segments with the same name (so the same bm25 score);
// only one carries house number 35. both are returned, but the segment that resolved the number
// ranks first — the similarity nudge breaks the score tie.
#[test]
fn _23_segment_with_house_number_outranks_the_bare_segment() {
  let s = scene::of(&[
    make_street_row("rua castro alves", 0.000, 1),
    make_street_row("rua castro alves", 0.001, 2),
  ]);
  let id2 = s.id_by_way(2);
  s.insert_house_number(id2, 1, "35", -46.31879, -23.97240);

  let out = s.ask("rua castro alves 35");
  assert_eq!(out.matches.len(), 2);

  let way_of = |i: usize| {
    out.matches[i]
      .admin_levels
      .iter()
      .find(|a| a.level == 12)
      .and_then(|a| a.osm_way_id)
  };
  let kind_of = |i: usize| out.matches[i].house_number.as_ref().map(|h| &h.kind);

  assert!(matches!(kind_of(0), Some(house_number_match::exact)));
  assert_eq!(way_of(0), Some(2));

  assert!(matches!(kind_of(1), Some(house_number_match::absent)));
  assert_eq!(way_of(1), Some(1));
}

// resolving the house number adds 0.01 to the match similarity; an absent number does not.
#[test]
fn _24_resolving_the_house_number_boosts_similarity() {
  let s = scene::numbered_street("rua oscar freire", &[("100", -46.31980, -23.97240)]);

  // base coverage for "rua oscar freire 100" is 3/4 = 0.75 (the number is uncovered)
  let resolved = s.ask("rua oscar freire 100");
  let m = resolved.matches.first().expect("expected one match");
  assert!(matches!(
    m.house_number.as_ref().map(|h| &h.kind),
    Some(house_number_match::exact)
  ));
  assert_eq!(m.similarity, Some(0.76));

  // an absent number leaves similarity untouched
  let absent = s.ask("rua oscar freire 999");
  let m = absent.matches.first().expect("expected one match");
  assert!(matches!(
    m.house_number.as_ref().map(|h| &h.kind),
    Some(house_number_match::absent)
  ));
  assert_eq!(m.similarity, Some(0.75));
}

#[test]
fn _25_admin_level_filter_keeps_only_requested_levels() {
  let street = make_street_row("embare", 0.000, 1);
  let mut city = make_street_row("embare", 0.001, 2);
  city.level = level::city;
  let s = scene::of(&[street, city]);

  let out_city = s.ask_with("embare", &with_levels(vec![level::city]));
  assert_eq!(out_city.matches.len(), 1);
  assert_eq!(out_city.matches[0].admin_levels.last().unwrap().level, 8);

  let out_both = s.ask_with("embare", &with_levels(vec![level::city, level::street]));
  assert_eq!(out_both.matches.len(), 2);
}

#[test]
fn _26_house_number_enriched_match_requires_level_30_in_the_filter() {
  let s = scene::numbered_street("rua oscar freire", &[("100", -46.31980, -23.97240)]);

  let out_street_only = s.ask_with("rua oscar freire 100", &with_levels(vec![level::street]));
  assert!(out_street_only.matches.is_empty());

  let out_with_30 = s.ask_with(
    "rua oscar freire 100",
    &with_levels(vec![level::street, level::house_number]),
  );
  assert_eq!(out_with_30.matches.len(), 1);
  let m = &out_with_30.matches[0];
  assert_eq!(m.admin_levels.last().unwrap().level, 30);
  assert_eq!(m.house_number.as_ref().unwrap().number, "100");
}

#[test]
fn _27_admin_level_filter_finds_levels_ranked_beyond_the_fts_limit() {
  let mut rows: Vec<admin_levels_row> = (1..=60)
    .map(|i| make_street_row("santos", 0.0001 * i as f64, i as u64))
    .collect();
  let mut city = make_street_row("santos", 0.01, 1000);
  city.level = level::city;
  rows.push(city);
  let s = scene::of(&rows);

  let out = s.ask_with("santos", &with_levels(vec![level::city]));
  assert_eq!(out.matches.len(), 1);
  assert_eq!(out.matches[0].admin_levels.last().unwrap().level, 8);
  assert_eq!(out.matches[0].admin_levels.last().unwrap().name, "santos");
}

#[test]
fn _28_bounding_wkt_keeps_in_region_match_ranked_beyond_fts_limit() {
  let mut rows: Vec<admin_levels_row> = Vec::new();
  // 55 exact "praca" outside the region (large offsets): high bm25, they fill the fts top 50
  for i in 1..=55u64 {
    rows.push(make_street_row("praca", 0.01 * i as f64, i));
  }
  // one "praca ..." inside the region (offset 0): long name, lower bm25, ranked beyond 50
  rows.push(make_street_row(
    "praca do mar shopping center jardim",
    0.0,
    100,
  ));
  let s = scene::of(&rows);

  let bounds = crate::http::parse_bounding_wkt(
    "POLYGON((-46.33 -23.98, -46.31 -23.98, -46.31 -23.96, -46.33 -23.96, -46.33 -23.98))",
  )
  .unwrap();
  let out = s.ask_with(
    "praca",
    &query_opts {
      bounding: Some(bounds),
      include_wkt: true,
      ..Default::default()
    },
  );
  assert_eq!(
    leaf_names(&out),
    vec!["praca do mar shopping center jardim"]
  );
}
