use crate::common::ask::ask;
use crate::common::harness::{encode, get, scenario, world, world_cell};
use crate::common::query::{first, levels_of, matches, name_at, way_ids};
use serde_json::{Value, json};

pub static SCENARIO: scenario = scenario {
  region: "02_preset_brazil",
  fixture_dir: "02_preset_brazil",
  pbf: "santos.osm.pbf",
  preset: "brazil",
};

static WORLD: world_cell = world_cell::new();

fn world() -> &'static world {
  WORLD.get(&SCENARIO)
}

const TEXT_QUERY: &str = "rua castro alves, embare, santos, sao paulo";
const COORDINATES_QUERY: &str = "-23.970949,-46.318730";

const INSIDE_POLYGON: &str =
  "POLYGON((-46.33 -23.98,-46.31 -23.98,-46.31 -23.96,-46.33 -23.96,-46.33 -23.98))";
const OUTSIDE_POLYGON: &str =
  "POLYGON((-45.0 -25.5,-44.9 -25.5,-44.9 -25.4,-45.0 -25.4,-45.0 -25.5))";

fn assert_boundary(relation_id: u64, level: u8, name: &str) {
  let conn = world().open_sqlite();
  let (stored_level, stored_name, wkb_len): (u8, String, i64) = conn
    .query_row(
      "SELECT admin_level, name, LENGTH(wkb) FROM admin_levels WHERE relation_id = ?1",
      [relation_id],
      |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap_or_else(|e| {
      panic!("relation {relation_id} ({name}) is missing from admin_levels: {e}")
    });
  assert_eq!(
    stored_level, level,
    "relation {relation_id} landed at the wrong level"
  );
  assert_eq!(
    stored_name, name,
    "relation {relation_id} has an unexpected name"
  );
  assert!(
    wkb_len > 0,
    "relation {relation_id} ({name}) has no geometry"
  );
}

// 00.00. result quality: reverse geocoding lands on the nearest street
#[test]
#[ignore]
fn _00_00_top_match_is_a_street_within_the_expected_distance() {
  let result = world().run(&[COORDINATES_QUERY]);
  let top = first(&result);
  let distance = top["coordinates_distance_in_meters"]
    .as_u64()
    .expect("a coordinate match must report a distance");
  let limit = 100;
  assert!(
    distance <= limit,
    "the closest street is {distance} m away, limit is {limit}"
  );
  assert!(
    levels_of(top).contains(&12),
    "a coordinate match must resolve down to a street"
  );
}

// 00.01. result quality
#[test]
#[ignore]
fn _00_01_top_match_carries_the_full_ladder() {
  let result = world().run(&[COORDINATES_QUERY]);
  let top = first(&result);
  for (level, name) in [
    (2, "Brasil"),
    (4, "São Paulo"),
    (8, "Santos"),
    (10, "Embaré"),
    (12, "Rua Castro Alves"),
  ] {
    assert_eq!(
      name_at(top, level).as_deref(),
      Some(name),
      "level {level} resolved to the wrong name"
    );
  }
}

// 00.02. result quality: the readme example is the executable proof of the documented address
#[test]
#[ignore]
fn _00_02_the_documented_example_resolves_to_the_documented_address() {
  let w = world();
  let result = w.assert_cli(
    &ask(TEXT_QUERY),
    &json!({
      "service": "text_to_address",
      "matches": [{
        "friendly_name": "Rua Castro Alves, Embaré, Santos, São Paulo, Brasil",
        "similarity": 1.0,
      }],
    }),
  );
  assert_eq!(levels_of(first(&result)), vec![2, 4, 8, 10, 12]);
}

// 00.03. result quality: the street is split in two ways; both must come back, order aside
#[test]
#[ignore]
fn _00_03_every_segment_of_the_street_comes_back() {
  // segments tie on score, so their relative order follows the tantivy segment layout.
  // assert the set, never the order.
  let mut expected = vec![255710390, 729205713];
  expected.sort_unstable();
  assert_eq!(way_ids(&world().run(&[TEXT_QUERY])), expected);
}

// 00.04. result quality
#[test]
#[ignore]
fn _00_04_a_house_number_on_the_street_resolves_as_exact() {
  // rua januario dos santos is a single segment carrying 70, 197, 232 and 235 — no tie to break,
  // so the ranking is stable.
  let w = world();
  let result = w.assert_cli(
    &ask("rua januario dos santos, santos 197"),
    &json!({
      "matches": [{
        "friendly_name": "Rua Januário dos Santos, 197, Aparecida, Santos, São Paulo, Brasil",
        "house_number": { "number": "197", "kind": "exact" },
      }],
    }),
  );
  assert!(levels_of(first(&result)).contains(&30));
}

// 00.05. result quality
#[test]
#[ignore]
fn _00_05_a_house_number_between_two_known_ones_resolves_as_interpolated() {
  let w = world();
  let result = w.assert_cli(
    &ask("rua januario dos santos, santos 210"),
    &json!({ "matches": [{ "house_number": { "number": "210", "kind": "interpolated" } }] }),
  );
  assert!(levels_of(first(&result)).contains(&30));
}

// 00.06. result quality
#[test]
#[ignore]
fn _00_06_an_out_of_range_house_number_resolves_as_absent() {
  let w = world();
  let bare = world().run(&["rua januario dos santos, santos"]);
  let numbered = w.assert_cli(
    &ask("rua januario dos santos, santos 99999"),
    &json!({ "matches": [{ "house_number": { "number": "99999", "kind": "absent" } }] }),
  );
  let top = first(&numbered);
  assert!(
    !levels_of(top).contains(&30),
    "an unresolved number must not become a level"
  );
  assert_eq!(
    top["latitude"],
    first(&bare)["latitude"],
    "an absent number must leave the street coordinate untouched"
  );
}

// 00.07. result quality
#[test]
#[ignore]
fn _00_07_friendly_name_format_renders_the_requested_placeholders() {
  let w = world();
  for (format, expected) in [
    (
      "{admin_level_12_name} - {admin_level_8_name}",
      "Rua Castro Alves - Santos",
    ),
    (
      "{admin_level_12_name}, {admin_level_6_name}, {admin_level_8_name}",
      "Rua Castro Alves, Santos",
    ),
  ] {
    w.assert_cli(
      &ask(TEXT_QUERY).friendly_name_format(format),
      &json!({ "matches": [{ "friendly_name": expected }] }),
    );
  }
}

// 00.08. result quality
#[test]
#[ignore]
fn _00_08_friendly_name_format_house_number_alias() {
  let w = world();
  w.assert_cli(
    &ask("rua januario dos santos, santos 197")
      .friendly_name_format("{admin_level_12_name} {house_number}"),
    &json!({ "matches": [{ "friendly_name": "Rua Januário dos Santos 197" }] }),
  );
}

// 00.09. result quality: the full payload for the documented text query, compared whole
#[test]
#[ignore]
fn _00_09_the_text_query_matches_are_exactly_these() {
  // every match, whole. the two segments tie on score, so the array is sorted by id before
  // comparing — their order follows the tantivy segment layout and must never be asserted.
  let result = world().run(&[TEXT_QUERY]);
  let mut sorted = matches(&result).clone();
  sorted.sort_by_key(|m| m["id"].as_u64().expect("id must be a number"));
  world().assert_exact(
    &Value::Array(sorted),
    &json!([
      {
        "admin_levels": [
          {
            "level": 2,
            "name": "Brasil",
            "osm_relation_id": 59470,
            "osm_way_id": null,
          },
          {
            "level": 4,
            "name": "São Paulo",
            "osm_relation_id": 298204,
            "osm_way_id": null,
          },
          {
            "level": 8,
            "name": "Santos",
            "osm_relation_id": 298442,
            "osm_way_id": null,
          },
          {
            "level": 10,
            "name": "Embaré",
            "osm_relation_id": 4282882,
            "osm_way_id": null,
          },
          {
            "level": 12,
            "name": "Rua Castro Alves",
            "osm_relation_id": null,
            "osm_way_id": 255710390,
          },
        ],
        "attributes": {
          "country_iso_3166_1_alpha_2_code": "BR",
          "post_code": null,
        },
        "coordinates_distance_in_meters": null,
        "friendly_name": "Rua Castro Alves, Embaré, Santos, São Paulo, Brasil",
        "id": 511420780,
        "latitude": -23.9718,
        "longitude": -46.3195,
        "score": 111.089,
        "similarity": 1.0,
      },
      {
        "admin_levels": [
          {
            "level": 2,
            "name": "Brasil",
            "osm_relation_id": 59470,
            "osm_way_id": null,
          },
          {
            "level": 4,
            "name": "São Paulo",
            "osm_relation_id": 298204,
            "osm_way_id": null,
          },
          {
            "level": 8,
            "name": "Santos",
            "osm_relation_id": 298442,
            "osm_way_id": null,
          },
          {
            "level": 10,
            "name": "Embaré",
            "osm_relation_id": 4282882,
            "osm_way_id": null,
          },
          {
            "level": 12,
            "name": "Rua Castro Alves",
            "osm_relation_id": null,
            "osm_way_id": 729205713,
          },
        ],
        "attributes": {
          "country_iso_3166_1_alpha_2_code": "BR",
          "post_code": null,
        },
        "coordinates_distance_in_meters": null,
        "friendly_name": "Rua Castro Alves, Embaré, Santos, São Paulo, Brasil",
        "id": 1458411426,
        "latitude": -23.9692,
        "longitude": -46.3173,
        "score": 111.089,
        "similarity": 1.0,
      },
    ]),
  );
}

// 00.10. result quality: the full top match for the documented coordinate, compared whole
#[test]
#[ignore]
fn _00_10_the_coordinate_query_top_match_is_exactly_this() {
  let actual = first(&world().run(&[COORDINATES_QUERY])).clone();
  world().assert_exact(
    &actual,
    &json!({
      "admin_levels": [
        {
          "level": 2,
          "name": "Brasil",
          "osm_relation_id": 59470,
          "osm_way_id": null,
        },
        {
          "level": 4,
          "name": "São Paulo",
          "osm_relation_id": 298204,
          "osm_way_id": null,
        },
        {
          "level": 8,
          "name": "Santos",
          "osm_relation_id": 298442,
          "osm_way_id": null,
        },
        {
          "level": 10,
          "name": "Embaré",
          "osm_relation_id": 4282882,
          "osm_way_id": null,
        },
        {
          "level": 12,
          "name": "Rua Castro Alves",
          "osm_relation_id": null,
          "osm_way_id": 729205713,
        },
        {
          "level": 30,
          "name": "35",
          "osm_relation_id": null,
          "osm_way_id": null,
        },
      ],
      "attributes": {
        "country_iso_3166_1_alpha_2_code": "BR",
        "post_code": null,
      },
      "coordinates_distance_in_meters": 3,
      "friendly_name": "Rua Castro Alves, 35, Embaré, Santos, São Paulo, Brasil",
      "id": 1458411426,
      "latitude": -23.9709,
      "longitude": -46.3188,
      "score": null,
      "similarity": null,
    }),
  );
}

// 00.11. result quality
#[test]
#[ignore]
fn _00_11_an_accented_query_is_url_decoded() {
  let w = world();
  let s = w.start_server();
  let r = get(s.port, &ask("rua castro alves, embaré, santos").http_path());
  assert_eq!(r.status, 200);
  assert!(
    !r.json()["matches"].as_array().expect("matches").is_empty(),
    "the accented spelling must be found"
  );
}

// 01.00. precision guarantee: only reachable when the level 2 ring closed
#[test]
#[ignore]
fn _01_00_the_country_iso_code_is_resolved() {
  let w = world();
  w.assert_cli(
    &ask(COORDINATES_QUERY),
    &json!({ "matches": [{ "attributes": { "country_iso_3166_1_alpha_2_code": "BR" } }] }),
  );
}

// 01.01. precision guarantee
#[test]
#[ignore]
fn _01_01_min_quality_one_keeps_only_fully_covered_matches() {
  let result = world().run(&[TEXT_QUERY, "--min-quality", "1.0"]);
  assert!(
    !matches(&result).is_empty(),
    "the exact query must survive its own floor"
  );
  for m in matches(&result) {
    assert_eq!(
      m["similarity"], 1.0,
      "a quality floor of 1.0 keeps only full coverage"
    );
  }
}

// 01.02. precision guarantee
#[test]
#[ignore]
fn _01_02_bounding_wkt_keeps_only_matches_inside_the_polygon() {
  let bounded = world().run(&[TEXT_QUERY, "--bounding-wkt", INSIDE_POLYGON]);
  assert!(
    !matches(&bounded).is_empty(),
    "the street lies inside the polygon"
  );
  assert_eq!(
    way_ids(&bounded),
    way_ids(&world().run(&[TEXT_QUERY])),
    "the polygon covers the whole street"
  );
}

// 01.03. precision guarantee
#[test]
#[ignore]
fn _01_03_a_bounding_polygon_over_open_water_excludes_everything() {
  let result = world().run(&[TEXT_QUERY, "--bounding-wkt", OUTSIDE_POLYGON]);
  assert!(
    matches(&result).is_empty(),
    "no street sits in that patch of water"
  );
}

// 01.04. precision guarantee
#[test]
#[ignore]
fn _01_04_last_admin_levels_keeps_only_the_requested_leaf() {
  let result = world().run(&["santos, sao paulo", "--last-admin-levels", "8"]);
  assert!(!matches(&result).is_empty());
  for m in matches(&result) {
    assert_eq!(
      levels_of(m).last().copied(),
      Some(8),
      "every match must end at the requested level"
    );
  }
}

// 02.00. ambiguity
#[test]
#[ignore]
fn _02_00_an_ascii_folded_query_matches_the_accented_name() {
  let folded = world().run(&["rua castro alves, embare, santos"]);
  let accented = world().run(&["rua castro alves, embaré, santos"]);
  assert_eq!(way_ids(&folded), way_ids(&accented));
  assert!(!way_ids(&folded).is_empty());
}

// 02.01. ambiguity
#[test]
#[ignore]
fn _02_01_the_preset_expands_the_street_abbreviation() {
  let abbreviated = world().run(&["r. castro alves, embare, santos"]);
  let spelled = world().run(&["rua castro alves, embare, santos"]);
  assert_eq!(way_ids(&abbreviated), way_ids(&spelled));
  assert!(!way_ids(&abbreviated).is_empty());
}

// 02.02. ambiguity: one expectation, both surfaces, and the two must agree byte for byte
#[test]
#[ignore]
fn _02_02_the_surfaces_agree_on_a_text_query() {
  let w = world();
  let s = w.start_server();
  w.assert_both(
    &s,
    &ask(TEXT_QUERY),
    &json!({
      "service": "text_to_address",
      "matches": [{ "friendly_name": "Rua Castro Alves, Embaré, Santos, São Paulo, Brasil" }],
    }),
  );
}

// 02.03. ambiguity
#[test]
#[ignore]
fn _02_03_the_surfaces_agree_on_a_coordinate_query() {
  let w = world();
  let s = w.start_server();
  w.assert_both(
    &s,
    &ask(COORDINATES_QUERY),
    &json!({ "service": "coordinates_to_address" }),
  );
}

// 02.04. ambiguity
#[test]
#[ignore]
fn _02_04_the_surfaces_agree_under_every_flag() {
  let w = world();
  let s = w.start_server();
  let query = TEXT_QUERY;
  let any = json!({ "service": "text_to_address" });

  w.assert_both(
    &s,
    &ask(query).friendly_name_format("{admin_level_12_name} - {admin_level_8_name}"),
    &json!({ "matches": [{ "friendly_name": "Rua Castro Alves - Santos" }] }),
  );
  w.assert_both(&s, &ask(query).min_quality("1.0"), &any);
  w.assert_both(&s, &ask(query).bounding_wkt(INSIDE_POLYGON), &any);
  w.assert_both(&s, &ask(query).last_admin_levels("12"), &any);
}

// 03.00. regression guard: a clipped boundary becomes a MultiLineString and can never be an ancestor
#[test]
#[ignore]
fn _03_00_boundary_relations_are_polygonal_not_linestrings() {
  for (relation_id, level, name) in [
    (59470, 2, "Brasil"),
    (298204, 4, "São Paulo"),
    (298442, 8, "Santos"),
  ] {
    assert_boundary(relation_id, level, name);
  }
}

// 03.01. regression guard: the iso code only resolves when the level 2 ring closed
#[test]
#[ignore]
fn _03_01_the_country_relation_carries_its_iso_code() {
  let w = world();
  let conn = w.open_sqlite();
  let stored: Option<String> = conn
    .query_row(
      "SELECT country_iso_code FROM admin_levels WHERE relation_id = 59470",
      [],
      |r| r.get(0),
    )
    .expect("the brazil relation must be in admin_levels");
  assert_eq!(stored.as_deref(), Some("BR"));
}

// 03.02. regression guard: a level 9 also named santos is in the fixture; the preset must drop it
#[test]
#[ignore]
fn _03_02_the_friendly_name_never_repeats_an_admin_level_name() {
  for m in matches(&world().run(&[TEXT_QUERY])) {
    let name = m["friendly_name"]
      .as_str()
      .expect("friendly_name must be a string");
    assert!(
      !name.contains("Santos, Santos"),
      "duplicated admin level in {name:?}"
    );
  }
}

// 03.03. regression guard: the leaf filter runs at retrieval (leaf 12) and again after enrichment (30)
#[test]
#[ignore]
fn _03_03_a_house_number_leaf_needs_both_the_street_and_the_house_number_level() {
  let query = "rua januario dos santos, santos 197";

  let street_only = world().run(&[query, "--last-admin-levels", "12"]);
  assert!(!matches(&street_only).is_empty());
  for m in matches(&street_only) {
    assert_eq!(
      levels_of(m).last().copied(),
      Some(12),
      "the enriched match is dropped: its leaf is 30, not 12"
    );
  }

  let both = world().run(&[query, "--last-admin-levels", "12,30"]);
  assert_eq!(
    levels_of(first(&both)).last().copied(),
    Some(30),
    "with both levels allowed, the enriched match ranks first"
  );

  let leaf_only = world().run(&[query, "--last-admin-levels", "30"]);
  assert!(
    matches(&leaf_only).is_empty(),
    "no indexed document has a level 30 leaf, so the search stage returns nothing"
  );
}

// 03.04. regression guard: force_content_length keeps a multi-megabyte body identity-encoded
#[test]
#[ignore]
fn _03_04_a_response_past_the_chunked_threshold_stays_identity_encoded() {
  let w = world();
  // the country ring pushes this response into megabytes, well past tiny_http's 32 KiB
  // chunked threshold. this is the regression test for force_content_length.
  let s = w.start_server();
  let r = get(
    s.port,
    &format!("/geocode?query={}", encode(COORDINATES_QUERY)),
  );
  assert_eq!(r.status, 200);
  assert!(
    r.body.len() > 32 * 1024,
    "expected a large body, got {} bytes",
    r.body.len()
  );
  assert!(
    r.header("transfer-encoding").is_none(),
    "the response must not be chunked"
  );
  assert_eq!(
    r.header("content-length")
      .and_then(|v| v.parse::<usize>().ok()),
    Some(r.body.len())
  );
}

// 04.00. pipeline integrity: the preset is inferred from the source path, not from a flag
#[test]
#[ignore]
fn _04_00_build_resolves_the_preset_from_the_source_path() {
  let w = world();
  let banner = format!("preset: {}", "brazil");
  assert!(
    w.build_stdout.contains(&banner),
    "expected {banner:?} to be inferred from the file name"
  );
}

// 04.01. pipeline integrity
#[test]
#[ignore]
fn _04_01_header_stage_reports_the_fixture_generator() {
  let w = world();
  assert!(
    w.build_stdout.contains("geolite-e2e-fixture/1"),
    "the header stage must report the generator written by build-fixture.sh"
  );
}

// 05.00. contract
#[test]
#[ignore]
fn _05_00_include_wkt_true_attaches_geometry_to_every_level() {
  let w = world();
  let result = w.query_json(&[TEXT_QUERY]);
  let top = first(&result);
  let wkt_at = |level: u64| -> String {
    top["admin_levels"]
      .as_array()
      .and_then(|a| a.iter().find(|l| l["level"].as_u64() == Some(level)))
      .and_then(|l| l["wkt"].as_str())
      .unwrap_or_else(|| panic!("level {level} must carry geometry"))
      .to_string()
  };
  assert!(
    wkt_at(12).starts_with("LINESTRING"),
    "streets are always lines"
  );

  let country = wkt_at(2);
  assert!(
    country.starts_with("MULTIPOLYGON"),
    "a closed country ring is a multipolygon"
  );
  assert!(
    country.len() > 1024 * 1024,
    "the brazil ring is megabytes; it is what makes the content-length contract testable"
  );
}

// 06.00. dead case
#[test]
#[ignore]
fn _06_00_a_typo_falls_back_to_the_loose_query() {
  assert!(
    !way_ids(&world().run(&["rua castro alvez, embare, santos"])).is_empty(),
    "the fuzzy fallback must still find the street"
  );
}
///////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////

// 02.05. ambiguity: ten spellings of the same address, every one landing on the same street
#[test]
#[ignore]
fn _02_05_every_spelling_of_the_address_lands_on_the_same_street() {
  let w = world();
  let s = w.start_server();
  for input in [
    "rua castro alves embare santos",
    "Rua Castro Alves, Embaré, Santos",
    "RUA CASTRO ALVES EMBARE SANTOS",
    "rua castro alves,embare,santos",
    "rua castro alves - embare - santos",
    "rua  castro   alves    embare  santos",
    "castro alves embare santos",
    "santos, embare, rua castro alves",
    "rua castro alves, embare, santos, sp",
    "Rua Castro Alves - Embaré - Santos/SP",
  ] {
    let result = w.assert_both(
      &s,
      &ask(input),
      &json!({
        "service": "text_to_address",
        "matches": [{ "friendly_name": "Rua Castro Alves, Embaré, Santos, São Paulo, Brasil" }],
      }),
    );
    assert_eq!(
      levels_of(first(&result)),
      vec![2, 4, 8, 10, 12],
      "{input:?} must resolve the full ladder"
    );
  }
}

// 02.06. ambiguity: ten points along the whole street, every one landing on the same street
#[test]
#[ignore]
fn _02_06_every_point_along_the_street_lands_on_the_same_street() {
  let w = world();
  let s = w.start_server();
  for input in [
    "-23.971817,-46.319467",
    "-23.971283,-46.319041",
    "-23.970752,-46.318611",
    "-23.970219,-46.318183",
    "-23.969685,-46.317756",
    "-23.969155,-46.317324",
    "-23.968826,-46.317054",
    "-23.968094,-46.316461",
    "-23.967564,-46.31603",
    "-23.967039,-46.315589",
  ] {
    w.assert_both(
      &s,
      &ask(input),
      &json!({
        "service": "coordinates_to_address",
        "matches": [{
          "admin_levels": [
            { "level": 2, "name": "Brasil" },
            { "level": 4, "name": "São Paulo" },
            { "level": 8, "name": "Santos" },
            { "level": 10, "name": "Embaré" },
            { "level": 12, "name": "Rua Castro Alves" },
          ],
        }],
      }),
    );
  }
}

// 02.07. ambiguity: ten spellings of the same square, every one landing on the same square
#[test]
#[ignore]
fn _02_07_every_spelling_of_the_square_lands_on_the_same_square() {
  let w = world();
  let s = w.start_server();
  for input in [
    "praca doutor hipolito do rego embare santos",
    "Praça Doutor Hipólito do Rego, Embaré, Santos",
    "PRACA DOUTOR HIPOLITO DO REGO EMBARE SANTOS",
    "praca doutor hipolito do rego,embare,santos",
    "praca doutor hipolito do rego - embare - santos",
    "pç. doutor hipolito do rego embare santos",
    "pca. doutor hipolito do rego embare santos",
    "praca dr. hipolito do rego embare santos",
    "santos, embare, praca doutor hipolito do rego",
    "Praça Doutor Hipólito do Rego - Embaré - Santos/SP",
  ] {
    let result = w.assert_both(
      &s,
      &ask(input),
      &json!({
        "service": "text_to_address",
        "matches": [{
          "friendly_name": "Praça Doutor Hipólito do Rego, Embaré, Santos, São Paulo, Brasil",
        }],
      }),
    );
    assert_eq!(
      levels_of(first(&result)),
      vec![2, 4, 8, 10, 12],
      "{input:?} must resolve the full ladder"
    );
  }
}

// 02.08. ambiguity: ten points along the whole square, every one landing on the same square
#[test]
#[ignore]
fn _02_08_every_point_along_the_square_lands_on_the_same_square() {
  let w = world();
  let s = w.start_server();
  for input in [
    "-23.972449,-46.319952",
    "-23.972405,-46.319919",
    "-23.972361,-46.319885",
    "-23.972316,-46.319852",
    "-23.972272,-46.319818",
    "-23.972227,-46.319785",
    "-23.972183,-46.319751",
    "-23.972091,-46.319785",
    "-23.972095,-46.319844",
    "-23.972075,-46.319895",
  ] {
    w.assert_both(
      &s,
      &ask(input),
      &json!({
        "service": "coordinates_to_address",
        "matches": [{
          "admin_levels": [
            { "level": 2, "name": "Brasil" },
            { "level": 4, "name": "São Paulo" },
            { "level": 8, "name": "Santos" },
            { "level": 10, "name": "Embaré" },
            { "level": 12, "name": "Praça Doutor Hipólito do Rego" },
          ],
        }],
      }),
    );
  }
}

// 02.09. ambiguity: ten spellings of the same numbered address, every one landing on the same house
#[test]
#[ignore]
fn _02_09_every_spelling_of_the_numbered_address_lands_on_the_same_house() {
  let w = world();
  let s = w.start_server();
  for input in [
    "rua doutor galeao carvalhal, 15 gonzaga santos",
    "Rua Doutor Galeão Carvalhal, 15, Gonzaga, Santos",
    "RUA DOUTOR GALEAO CARVALHAL 15 GONZAGA SANTOS",
    "rua doutor galeao carvalhal 15 - gonzaga - santos",
    "rua dr. galeao carvalhal, 15, gonzaga, santos",
    "r. dr. galeao carvalhal, 15, gonzaga, santos",
    "rua doutor galeao carvalhal, nº 15, gonzaga, santos",
    "15 rua doutor galeao carvalhal gonzaga santos",
    "santos, gonzaga, rua doutor galeao carvalhal, 15",
    "Rua Doutor Galeão Carvalhal, 15 - Gonzaga - Santos/SP",
  ] {
    let result = w.assert_both(
      &s,
      &ask(input),
      &json!({
        "service": "text_to_address",
        "matches": [{
          "friendly_name": "Rua Doutor Galeão Carvalhal, 15, Gonzaga, Santos, São Paulo, Brasil",
          "house_number": { "number": "15" },
        }],
      }),
    );
    assert_eq!(
      levels_of(first(&result)),
      vec![2, 4, 8, 10, 12, 30],
      "{input:?} must resolve the full ladder down to the house number"
    );
  }
}

// 02.10. ambiguity: five points along the whole street, every one landing on the same street
#[test]
#[ignore]
fn _02_10_every_point_along_the_numbered_street_lands_on_the_same_street() {
  let w = world();
  let s = w.start_server();
  for input in [
    "-23.96748,-46.332242",
    "-23.967571,-46.33156",
    "-23.967695,-46.330486",
    "-23.967803,-46.329608",
    "-23.96785,-46.329218",
  ] {
    w.assert_both(
      &s,
      &ask(input),
      &json!({
        "service": "coordinates_to_address",
        "matches": [{
          "admin_levels": [
            { "level": 2, "name": "Brasil" },
            { "level": 4, "name": "São Paulo" },
            { "level": 8, "name": "Santos" },
            { "level": 10, "name": "Gonzaga" },
            { "level": 12, "name": "Rua Doutor Galeão Carvalhal" },
          ],
        }],
      }),
    );
  }
}

// TODO: pegar 7 enderecos reais e garantir o resultado.
// TODO: pegar 7 coordenadas reais e garantir o resultado.
// TODO: revisar e apagar testes pre-gerados
