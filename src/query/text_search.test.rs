use crate::database::admin_levels::{admin_levels as admin_levels_row, batch_upsert};
use crate::index::admin_levels_hierarchy_tantivy::testing::build_test_index;
use geo::{Coord, Geometry, LineString, Point};

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
    admin_level: 12,
    wkb: Geometry::LineString(ls).into(),
    name: name.to_string(),
    country_iso_code: None,
    post_code: post_code.map(str::to_string),
  }
}

fn street_id(conn: &rusqlite::Connection, name: &str) -> i64 {
  conn
    .query_row("SELECT id FROM admin_levels WHERE name = ?1", [name], |r| {
      r.get(0)
    })
    .expect("failed to read street id")
}

fn id_by_way(conn: &rusqlite::Connection, way_id: u64) -> i64 {
  conn
    .query_row(
      "SELECT id FROM admin_levels WHERE way_id = ?1",
      [way_id],
      |r| r.get(0),
    )
    .expect("failed to read id by way")
}

fn insert_house_number(
  conn: &rusqlite::Connection,
  admin_level_id: i64,
  node_id: u64,
  number: &str,
  lon: f64,
  lat: f64,
) {
  let row = crate::database::house_numbers::house_numbers {
    node_id,
    admin_level_id,
    number: number.to_string(),
    wkb: Geometry::Point(Point::new(lon, lat)).into(),
    strategy: 0,
  };
  crate::database::house_numbers::batch_insert(conn, &[row]);
}

#[test]
fn _00_text_search_prioritizes_exact_case_match() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![
    make_street_row("aaa", 0.000, 1),
    make_street_row("AAA", 0.001, 2),
    make_street_row("BBB", 0.002, 3),
    make_street_row("bbb", 0.003, 4),
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let out_a = crate::query::run(&conn, Some(&index), "AAA", None, None, None, None, true);
  let names_a: Vec<String> = out_a
    .matches
    .iter()
    .filter_map(|m| m.admin_levels.last().map(|al| al.name.clone()))
    .collect();
  assert_eq!(names_a, vec!["AAA", "aaa"]);

  let out_b = crate::query::run(&conn, Some(&index), "bbb", None, None, None, None, true);
  let names_b: Vec<String> = out_b
    .matches
    .iter()
    .filter_map(|m| m.admin_levels.last().map(|al| al.name.clone()))
    .collect();
  assert_eq!(names_b, vec!["bbb", "BBB"]);
}

#[test]
fn _01_text_search_prioritizes_closest_spelling_variant() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![
    make_street_row("Brasil", 0.000, 1),
    make_street_row("Brazil", 0.001, 2),
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let out_brasil = crate::query::run(&conn, Some(&index), "brasil", None, None, None, None, true);
  let names_brasil: Vec<String> = out_brasil
    .matches
    .iter()
    .filter_map(|m| m.admin_levels.last().map(|al| al.name.clone()))
    .collect();
  assert_eq!(names_brasil, vec!["Brasil"]);

  let out_brazil = crate::query::run(&conn, Some(&index), "brazil", None, None, None, None, true);
  let names_brazil: Vec<String> = out_brazil
    .matches
    .iter()
    .filter_map(|m| m.admin_levels.last().map(|al| al.name.clone()))
    .collect();
  assert_eq!(names_brazil, vec!["Brazil"]);
}

#[test]
fn _03_text_search_prioritizes_matching_diacritic_variant() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![
    make_street_row("Praça", 0.000, 1),
    make_street_row("Praca", 0.001, 2),
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let out_praca_diacritico =
    crate::query::run(&conn, Some(&index), "praça", None, None, None, None, true);
  let names_diacritico: Vec<String> = out_praca_diacritico
    .matches
    .iter()
    .filter_map(|m| m.admin_levels.last().map(|al| al.name.clone()))
    .collect();
  assert_eq!(names_diacritico, vec!["Praça", "Praca"]);

  let out_praca_simples =
    crate::query::run(&conn, Some(&index), "Praca", None, None, None, None, true);
  let names_simples: Vec<String> = out_praca_simples
    .matches
    .iter()
    .filter_map(|m| m.admin_levels.last().map(|al| al.name.clone()))
    .collect();
  assert_eq!(names_simples, vec!["Praca", "Praça"]);
}

#[test]
fn _04_text_search_prioritizes_record_with_matching_hierarchy() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![
    make_street_row("rua castro alves", 0.000, 1),
    make_street_row("rua castro alves, embare", 0.001, 2),
    make_street_row("rua castro alves, embare, santos", 0.002, 3),
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let first_match = |input: &str| -> Option<String> {
    crate::query::run(&conn, Some(&index), input, None, None, None, None, true)
      .matches
      .first()
      .map(|m| m.friendly_name.clone())
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
fn _05_text_search_prioritizes_matching_word_order() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![
    make_street_row("embare, rua castro alves", 0.000, 1),
    make_street_row("alves castro rua embare", 0.001, 2),
    make_street_row("rua castro alves embare", 0.002, 3),
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let mut out = crate::query::run(
    &conn,
    Some(&index),
    "rua castro alves, embare",
    None,
    None,
    None,
    None,
    true,
  );
  let mut first = out.matches.first().map(|m| m.friendly_name.clone());
  assert_eq!(first.as_deref(), Some("rua castro alves embare"));

  out = crate::query::run(
    &conn,
    Some(&index),
    "embare, rua castro alves",
    None,
    None,
    None,
    None,
    true,
  );
  first = out.matches.first().map(|m| m.friendly_name.clone());
  assert_eq!(first.as_deref(), Some("embare, rua castro alves"));
}

#[test]
fn _06_text_search_finds_street_by_postcode_with_hyphen() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![
    make_street_row_with_postcode("av paulista", 0.000, 1, Some("01310-100")),
    make_street_row_with_postcode("rua oscar freire", 0.001, 2, Some("01426-001")),
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let out = crate::query::run(
    &conn,
    Some(&index),
    "01310-100",
    None,
    None,
    None,
    None,
    true,
  );
  let names: Vec<String> = out
    .matches
    .iter()
    .filter_map(|m| m.admin_levels.last().map(|al| al.name.clone()))
    .collect();
  assert!(names.contains(&"av paulista".to_string()));
  assert!(!names.contains(&"rua oscar freire".to_string()));
}

#[test]
fn _07_text_search_finds_street_by_postcode_without_hyphen() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![
    make_street_row_with_postcode("av paulista", 0.000, 1, Some("01310-100")),
    make_street_row_with_postcode("rua oscar freire", 0.001, 2, Some("01426-001")),
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let out = crate::query::run(
    &conn,
    Some(&index),
    "01310100",
    None,
    None,
    None,
    None,
    true,
  );
  let names: Vec<String> = out
    .matches
    .iter()
    .filter_map(|m| m.admin_levels.last().map(|al| al.name.clone()))
    .collect();
  assert!(names.contains(&"av paulista".to_string()));
}

#[test]
fn _08_friendly_name_contains_postcode_when_street_has_one() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row_with_postcode(
    "av paulista",
    0.000,
    1,
    Some("01310-100"),
  )];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let out = crate::query::run(
    &conn,
    Some(&index),
    "av paulista",
    None,
    None,
    None,
    None,
    true,
  );
  let first = out.matches.first().expect("expected one match");
  assert_eq!(first.friendly_name, "av paulista, 01310-100");
}

#[test]
fn _09_friendly_name_omits_postcode_when_street_has_none() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row("av paulista", 0.000, 1)];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let out = crate::query::run(
    &conn,
    Some(&index),
    "av paulista",
    None,
    None,
    None,
    None,
    true,
  );
  let first = out.matches.first().expect("expected one match");
  assert_eq!(first.friendly_name, "av paulista");
}

#[test]
fn _10_text_search_finds_match_when_input_has_extra_leading_word() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row("sehrs", 0.000, 1)];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let output = crate::query::run(
    &conn,
    Some(&index),
    "hotel sehrs",
    None,
    None,
    None,
    None,
    true,
  );
  assert_eq!(output.matches.len(), 1);
}

#[test]
fn _11_text_search_finds_match_when_input_has_extra_trailing_word() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row("rua oscar freire", 0.000, 1)];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let output = crate::query::run(
    &conn,
    Some(&index),
    "rua oscar freire ipanema",
    None,
    None,
    None,
    None,
    true,
  );
  assert_eq!(output.matches.len(), 1);
}

#[test]
fn _12_text_search_rank_results_as_expected() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![
    make_street_row("sehrs", 0.000, 1),
    make_street_row("h seh", 0.002, 2),
    make_street_row("hot hrs", 0.003, 3),
    make_street_row("hot sehrs", 0.004, 4),
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let output = crate::query::run(
    &conn,
    Some(&index),
    "hotel sehrs",
    None,
    None,
    None,
    None,
    true,
  );
  let names: Vec<String> = output
    .matches
    .iter()
    .filter_map(|m| m.admin_levels.last().map(|al| al.name.clone()))
    .collect();
  assert_eq!(names, vec!["hot sehrs", "sehrs", "hot hrs", "h seh"]);
}

#[test]
fn _13_similarity_reflects_match_quality() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![
    make_street_row_with_postcode("rua castro alves", 0.000, 1, Some("11250-000")),
    make_street_row("Praça da Sé", 0.002, 3),
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let outputs = &[
    crate::query::run(
      &conn,
      Some(&index),
      "rua castro alves",
      None,
      None,
      None,
      None,
      true,
    ),
    crate::query::run(&conn, Some(&index), "rua", None, None, None, None, true),
    crate::query::run(&conn, Some(&index), "castro", None, None, None, None, true),
    crate::query::run(&conn, Some(&index), "alves", None, None, None, None, true),
    crate::query::run(
      &conn,
      Some(&index),
      "rua castro",
      None,
      None,
      None,
      None,
      true,
    ),
    crate::query::run(
      &conn,
      Some(&index),
      "castro alves",
      None,
      None,
      None,
      None,
      true,
    ),
    crate::query::run(
      &conn,
      Some(&index),
      "rua alves",
      None,
      None,
      None,
      None,
      true,
    ),
    crate::query::run(
      &conn,
      Some(&index),
      "alves castro rua",
      None,
      None,
      None,
      None,
      true,
    ),
    crate::query::run(
      &conn,
      Some(&index),
      "rua castro alvez",
      None,
      None,
      None,
      None,
      true,
    ),
    crate::query::run(
      &conn,
      Some(&index),
      "11250-000",
      None,
      None,
      None,
      None,
      true,
    ),
    crate::query::run(
      &conn,
      Some(&index),
      "hotel rua castro alves",
      None,
      None,
      None,
      None,
      true,
    ),
    crate::query::run(
      &conn,
      Some(&index),
      "praca da se",
      None,
      None,
      None,
      None,
      true,
    ),
    crate::query::run(
      &conn,
      Some(&index),
      "11250000",
      None,
      None,
      None,
      None,
      true,
    ),
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
fn _14_text_search_praca_doutor_hipolito_do_rego_embare_santos() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![
    make_street_row("Praça Doutor Hipólito do Rego", 0.000, 1),
    make_street_row("Praça Santo Antônio do Embaré", 0.001, 2),
    make_street_row("Santo Antônio do Embaré", 0.002, 3),
    make_street_row("Rua Manoel Hipolito do Rego", 0.003, 4),
    make_street_row("Praça Hipólito Fernandes", 0.004, 5),
    make_street_row("Embaré", 0.005, 6),
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let out = crate::query::run(
    &conn,
    Some(&index),
    "praca doutor hipolito do rego embare santos",
    None,
    None,
    None,
    None,
    true,
  );

  let names: Vec<String> = out
    .matches
    .iter()
    .filter_map(|m| m.admin_levels.last().map(|al| al.name.clone()))
    .collect();
  assert_eq!(
    names,
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
fn _15_text_search_with_trailing_house_number_resolves_exact_point() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row("rua oscar freire", 0.000, 1)];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let id = street_id(&conn, "rua oscar freire");
  insert_house_number(&conn, id, 1, "100", -46.31980, -23.97240);
  insert_house_number(&conn, id, 2, "200", -46.31960, -23.97220);
  insert_house_number(&conn, id, 3, "300", -46.31940, -23.97200);
  let (_index_guard, index) = build_test_index(&conn);

  let out = crate::query::run(
    &conn,
    Some(&index),
    "rua oscar freire 100",
    None,
    None,
    None,
    None,
    true,
  );
  let m = out.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert!(matches!(hn.kind, crate::query::house_number_match::exact));
  assert_eq!(hn.number, "100");
  assert_eq!(m.latitude, -23.97240);
  assert_eq!(m.longitude, -46.31980);
  let last = m.admin_levels.last().expect("expected level");
  assert_eq!(last.level, 30);
  assert_eq!(last.name, "100");
  assert_eq!(m.friendly_name, "rua oscar freire, 100");
}

#[test]
fn _16_text_search_with_leading_house_number_resolves_exact_point() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row("rua oscar freire", 0.000, 1)];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let id = street_id(&conn, "rua oscar freire");
  insert_house_number(&conn, id, 1, "100", -46.31980, -23.97240);
  insert_house_number(&conn, id, 2, "200", -46.31960, -23.97220);
  insert_house_number(&conn, id, 3, "300", -46.31940, -23.97200);
  let (_index_guard, index) = build_test_index(&conn);

  let out = crate::query::run(
    &conn,
    Some(&index),
    "100 rua oscar freire",
    None,
    None,
    None,
    None,
    true,
  );
  let m = out.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert!(matches!(hn.kind, crate::query::house_number_match::exact));
  assert_eq!(m.latitude, -23.97240);
  assert_eq!(m.longitude, -46.31980);
}

#[test]
fn _17_text_search_with_missing_house_number_interpolates_between_neighbors() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row("rua oscar freire", 0.000, 1)];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let id = street_id(&conn, "rua oscar freire");
  insert_house_number(&conn, id, 1, "100", -46.31980, -23.97240);
  insert_house_number(&conn, id, 2, "200", -46.31960, -23.97220);
  insert_house_number(&conn, id, 3, "300", -46.31940, -23.97200);
  let (_index_guard, index) = build_test_index(&conn);

  let out = crate::query::run(
    &conn,
    Some(&index),
    "rua oscar freire 150",
    None,
    None,
    None,
    None,
    true,
  );
  let m = out.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert!(matches!(
    hn.kind,
    crate::query::house_number_match::interpolated
  ));
  assert_eq!(hn.number, "150");
  assert_eq!(m.latitude, -23.97230);
  assert_eq!(m.longitude, -46.31970);
  assert_eq!(m.friendly_name, "rua oscar freire, 150");
}

#[test]
fn _18_text_search_with_unbracketable_house_number_marks_absent() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row("rua oscar freire", 0.000, 1)];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let id = street_id(&conn, "rua oscar freire");
  insert_house_number(&conn, id, 1, "100", -46.31980, -23.97240);
  insert_house_number(&conn, id, 2, "200", -46.31960, -23.97220);
  insert_house_number(&conn, id, 3, "300", -46.31940, -23.97200);
  let (_index_guard, index) = build_test_index(&conn);

  let out = crate::query::run(
    &conn,
    Some(&index),
    "rua oscar freire 400",
    None,
    None,
    None,
    None,
    true,
  );
  let m = out.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert!(matches!(hn.kind, crate::query::house_number_match::absent));
  let last = m.admin_levels.last().expect("expected level");
  assert_eq!(last.level, 12);
  assert_eq!(m.friendly_name, "rua oscar freire");
}

#[test]
fn _19_text_search_by_postcode_does_not_strip_a_house_number() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![
    make_street_row_with_postcode("av paulista", 0.000, 1, Some("01310-100")),
    make_street_row_with_postcode("rua oscar freire", 0.001, 2, Some("01426-001")),
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  for q in ["01310-100", "01310100"] {
    let out = crate::query::run(&conn, Some(&index), q, None, None, None, None, true);
    let m = out.matches.first().expect("expected one match");
    assert!(m.house_number.is_none());
    assert_eq!(
      m.admin_levels.last().map(|a| a.name.as_str()),
      Some("av paulista")
    );
  }
}

#[test]
fn _20_text_search_ignores_a_street_name_number_and_resolves_the_house_number() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row("rua 25 de marco", 0.000, 1)];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let id = street_id(&conn, "rua 25 de marco");
  insert_house_number(&conn, id, 1, "100", -46.31980, -23.97240);
  let (_index_guard, index) = build_test_index(&conn);

  let out = crate::query::run(
    &conn,
    Some(&index),
    "rua 25 de marco 100",
    None,
    None,
    None,
    None,
    true,
  );
  let m = out.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert_eq!(hn.number, "100");
  assert!(matches!(hn.kind, crate::query::house_number_match::exact));
  assert_eq!(
    m.admin_levels
      .iter()
      .find(|a| a.level == 12)
      .map(|a| a.name.as_str()),
    Some("rua 25 de marco"),
  );
}

// the house number is resolved the same way whether it comes right after the street name
// (brazilian format "rua x 35, bairro, cidade") or at the very end of the query.
#[test]
fn _21_house_number_resolves_the_same_in_the_middle_or_at_the_end() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row(
    "rua castro alves, embare, santos",
    0.000,
    1,
  )];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let id = street_id(&conn, "rua castro alves, embare, santos");
  insert_house_number(&conn, id, 1, "35", -46.31980, -23.97240);
  let (_index_guard, index) = build_test_index(&conn);

  for q in [
    "rua castro alves, embare, santos 35",
    "rua castro alves 35, embare, santos",
  ] {
    let out = crate::query::run(&conn, Some(&index), q, None, None, None, None, true);
    let m = out.matches.first().expect("expected one match");
    let hn = m.house_number.as_ref().expect("expected house_number");
    assert!(matches!(hn.kind, crate::query::house_number_match::exact));
    assert_eq!(hn.number, "35");
    assert_eq!(m.latitude, -23.97240);
    assert_eq!(m.longitude, -46.31980);
  }
}

// only the first numeric occurrence after the street name is taken as the house number — not
// whichever number happens to exist on the street. here "50" comes first, so the existing
// "200" is ignored and the result is absent (50 can't be placed).
#[test]
fn _22_uses_only_the_first_numeric_occurrence_after_the_street_name() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row("rua oscar freire", 0.000, 1)];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let id = street_id(&conn, "rua oscar freire");
  insert_house_number(&conn, id, 1, "200", -46.31960, -23.97220);
  let (_index_guard, index) = build_test_index(&conn);

  let out = crate::query::run(
    &conn,
    Some(&index),
    "rua oscar freire 50 200",
    None,
    None,
    None,
    None,
    true,
  );
  let m = out.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert_eq!(hn.number, "50");
  assert!(matches!(hn.kind, crate::query::house_number_match::absent));
}

// a street whose own name holds several numbers ("25" and "2024"): all of them are removed
// before picking the house number, so only a real trailing number resolves, and a query that
// is just the street name (no house number) resolves to no house_number at all.
#[test]
fn _23_ignores_every_number_that_belongs_to_the_street_name() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row("rua 25 de marco de 2024", 0.000, 1)];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let id = street_id(&conn, "rua 25 de marco de 2024");
  insert_house_number(&conn, id, 1, "100", -46.31980, -23.97240);
  let (_index_guard, index) = build_test_index(&conn);

  let with_number = crate::query::run(
    &conn,
    Some(&index),
    "rua 25 de marco de 2024 100",
    None,
    None,
    None,
    None,
    true,
  );
  let m = with_number.matches.first().expect("expected one match");
  let hn = m.house_number.as_ref().expect("expected house_number");
  assert!(matches!(hn.kind, crate::query::house_number_match::exact));
  assert_eq!(hn.number, "100");
  assert_eq!(
    m.admin_levels
      .iter()
      .find(|a| a.level == 12)
      .map(|a| a.name.as_str()),
    Some("rua 25 de marco de 2024"),
  );

  let without_number = crate::query::run(
    &conn,
    Some(&index),
    "rua 25 de marco de 2024",
    None,
    None,
    None,
    None,
    true,
  );
  let m = without_number.matches.first().expect("expected one match");
  assert!(m.house_number.is_none());
}

// the same street is split into two osm segments with the same name (so the same bm25 score);
// only one carries house number 35. both are returned, but the segment that resolved the number
// ranks first — the similarity nudge breaks the score tie.
#[test]
fn _24_segment_with_house_number_outranks_the_bare_segment() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![
    make_street_row("rua castro alves", 0.000, 1),
    make_street_row("rua castro alves", 0.001, 2),
  ];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let id2 = id_by_way(&conn, 2);
  insert_house_number(&conn, id2, 1, "35", -46.31879, -23.97240);
  let (_index_guard, index) = build_test_index(&conn);

  let out = crate::query::run(
    &conn,
    Some(&index),
    "rua castro alves 35",
    None,
    None,
    None,
    None,
    true,
  );
  assert_eq!(out.matches.len(), 2);

  let first = &out.matches[0];
  assert!(matches!(
    first.house_number.as_ref().map(|h| &h.kind),
    Some(crate::query::house_number_match::exact)
  ));
  assert_eq!(
    first
      .admin_levels
      .iter()
      .find(|a| a.level == 12)
      .and_then(|a| a.osm_way_id),
    Some(2),
  );

  let second = &out.matches[1];
  assert!(matches!(
    second.house_number.as_ref().map(|h| &h.kind),
    Some(crate::query::house_number_match::absent)
  ));
  assert_eq!(
    second
      .admin_levels
      .iter()
      .find(|a| a.level == 12)
      .and_then(|a| a.osm_way_id),
    Some(1),
  );
}

// resolving the house number adds 0.01 to the match similarity; an absent number does not.
#[test]
fn _25_resolving_the_house_number_boosts_similarity() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row("rua oscar freire", 0.000, 1)];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let id = street_id(&conn, "rua oscar freire");
  insert_house_number(&conn, id, 1, "100", -46.31980, -23.97240);
  let (_index_guard, index) = build_test_index(&conn);

  // base coverage for "rua oscar freire 100" is 3/4 = 0.75 (the number is uncovered)
  let resolved = crate::query::run(
    &conn,
    Some(&index),
    "rua oscar freire 100",
    None,
    None,
    None,
    None,
    true,
  );
  let m = resolved.matches.first().expect("expected one match");
  assert!(matches!(
    m.house_number.as_ref().map(|h| &h.kind),
    Some(crate::query::house_number_match::exact)
  ));
  assert_eq!(m.similarity, Some(0.76));

  // an absent number leaves similarity untouched
  let absent = crate::query::run(
    &conn,
    Some(&index),
    "rua oscar freire 999",
    None,
    None,
    None,
    None,
    true,
  );
  let m = absent.matches.first().expect("expected one match");
  assert!(matches!(
    m.house_number.as_ref().map(|h| &h.kind),
    Some(crate::query::house_number_match::absent)
  ));
  assert_eq!(m.similarity, Some(0.75));
}

#[test]
fn _26_admin_level_filter_keeps_only_requested_levels() {
  let conn = crate::database::open_write(":memory:");
  let street = make_street_row("embare", 0.000, 1);
  let mut city = make_street_row("embare", 0.001, 2);
  city.admin_level = 8;
  batch_upsert(&conn, &[street, city]);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let out_city = crate::query::run(
    &conn,
    Some(&index),
    "embare",
    None,
    None,
    None,
    Some(vec![8]),
    true,
  );
  assert_eq!(out_city.matches.len(), 1);
  assert_eq!(out_city.matches[0].admin_levels.last().unwrap().level, 8);

  let out_both = crate::query::run(
    &conn,
    Some(&index),
    "embare",
    None,
    None,
    None,
    Some(vec![8, 12]),
    true,
  );
  assert_eq!(out_both.matches.len(), 2);
}

#[test]
fn _27_house_number_enriched_match_requires_level_30_in_the_filter() {
  let conn = crate::database::open_write(":memory:");
  let rows = vec![make_street_row("rua oscar freire", 0.000, 1)];
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let id = street_id(&conn, "rua oscar freire");
  insert_house_number(&conn, id, 1, "100", -46.31980, -23.97240);
  let (_index_guard, index) = build_test_index(&conn);

  let out_street_only = crate::query::run(
    &conn,
    Some(&index),
    "rua oscar freire 100",
    None,
    None,
    None,
    Some(vec![12]),
    true,
  );
  assert!(out_street_only.matches.is_empty());

  let out_with_30 = crate::query::run(
    &conn,
    Some(&index),
    "rua oscar freire 100",
    None,
    None,
    None,
    Some(vec![12, 30]),
    true,
  );
  assert_eq!(out_with_30.matches.len(), 1);
  let m = &out_with_30.matches[0];
  assert_eq!(m.admin_levels.last().unwrap().level, 30);
  assert_eq!(m.house_number.as_ref().unwrap().number, "100");
}

#[test]
fn _28_admin_level_filter_finds_levels_ranked_beyond_the_fts_limit() {
  let conn = crate::database::open_write(":memory:");
  let mut rows: Vec<admin_levels_row> = (1..=60)
    .map(|i| make_street_row("santos", 0.0001 * i as f64, i as u64))
    .collect();
  let mut city = make_street_row("santos", 0.01, 1000);
  city.admin_level = 8;
  rows.push(city);
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let out = crate::query::run(
    &conn,
    Some(&index),
    "santos",
    None,
    None,
    None,
    Some(vec![8]),
    true,
  );
  assert_eq!(out.matches.len(), 1);
  assert_eq!(out.matches[0].admin_levels.last().unwrap().level, 8);
  assert_eq!(out.matches[0].admin_levels.last().unwrap().name, "santos");
}

#[test]
fn _29_include_wkt_emits_wkt_per_admin_level() {
  // todo!()
}

#[test]
fn _30_bounding_wkt_keeps_in_region_match_ranked_beyond_fts_limit() {
  let conn = crate::database::open_write(":memory:");
  let mut rows: Vec<admin_levels_row> = Vec::new();
  // 55 "praca" exatas fora do recorte (offsets grandes) — alto bm25, ocupam o top-50 do fts
  for i in 1..=55u64 {
    rows.push(make_street_row("praca", 0.01 * i as f64, i));
  }
  // 1 "praca ..." dentro do recorte (offset 0) — nome longo, bm25 menor, ranqueada alem de 50
  rows.push(make_street_row(
    "praca do mar shopping center jardim",
    0.0,
    100,
  ));
  batch_upsert(&conn, &rows);
  crate::index::coordinates::run(&conn, |_| {});
  crate::index::hierarchy::run(&conn, |_| {});
  let (_index_guard, index) = build_test_index(&conn);

  let bounds = crate::http::parse_bounding_wkt(
    "POLYGON((-46.33 -23.98, -46.31 -23.98, -46.31 -23.96, -46.33 -23.96, -46.33 -23.98))",
  )
  .unwrap();
  let out = crate::query::run(
    &conn,
    Some(&index),
    "praca",
    None,
    None,
    Some(bounds),
    None,
    true,
  );
  let names: Vec<String> = out
    .matches
    .iter()
    .filter_map(|m| m.admin_levels.last().map(|al| al.name.clone()))
    .collect();
  assert_eq!(names, vec!["praca do mar shopping center jardim"]);
}
