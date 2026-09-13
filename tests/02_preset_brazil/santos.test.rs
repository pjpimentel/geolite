use crate::common::ask::ask;
use crate::common::harness::{encode, get, scenario, world, world_cell};
use crate::common::query::{
  distances, first, leaves, level_at, levels_of, matches, name_at, names_at, point_of, way_ids,
  wkt_at,
};
use geo::Geometry;
use geozero::{ToGeo, wkb::SpatiaLiteWkb};
use serde_json::{Value, json};

pub static SCENARIO: scenario = scenario {
  region: "02_preset_brazil",
  fixture_dir: "02_preset_brazil",
  pbf: "santos.osm.pbf",
  preset: "brazil",
};

pub(crate) static WORLD: world_cell = world_cell::new();

pub(crate) fn world() -> &'static world {
  WORLD.get(&SCENARIO)
}

const TEXT_QUERY: &str = "rua castro alves, embare, santos, sao paulo";
const COORDINATES_QUERY: &str = "-23.970949,-46.318730";

const INSIDE_POLYGON: &str =
  "POLYGON((-46.33 -23.98,-46.31 -23.98,-46.31 -23.96,-46.33 -23.96,-46.33 -23.98))";
const OUTSIDE_POLYGON: &str =
  "POLYGON((-45.0 -25.5,-44.9 -25.5,-44.9 -25.4,-45.0 -25.4,-45.0 -25.5))";

// the point of house number 197 on a single-segment street carrying the numbers 70, 197, 232, 235
const NUMBERED_POINT: &str = "-23.98202,-46.31005";
const NUMBERED_STREET_NAME: &str = "Rua Januário dos Santos";
const APARECIDA_POLYGON: &str = "POLYGON((-46.3110 -23.9830,-46.3090 -23.9830,-46.3090 -23.9810,-46.3110 -23.9810,-46.3110 -23.9830))";
// ~700 m east of the numbered point: it holds one street, and that street is not among the ten
// nearest to the point
const FAR_POLYGON: &str = "POLYGON((-46.3040 -23.9830,-46.3020 -23.9830,-46.3020 -23.9810,-46.3040 -23.9810,-46.3040 -23.9830))";
const POST_CODED_STREET: &str = "Ateneu São Vicente";
const POST_CODED_POINT: &str = "-23.96675,-46.37675";
// rua aureliano coutinho lies inside conjunto habitacional jaú, which lies inside aparecida: two
// ancestors of level 10
const NESTED_POINT: &str = "-23.973439,-46.309747";
const NESTED_QUERY: &str = "rua aureliano coutinho, conjunto habitacional jau, santos";

const SQL_SELECT_BOUNDARY: &str = "
  SELECT admin_level, name, wkb
  FROM admin_levels
  WHERE relation_id = ?1
";

const SQL_SELECT_COUNTRY_ISO_CODE: &str = "
  SELECT country_iso_code
  FROM admin_levels
  WHERE relation_id = 59470
";

fn assert_boundary(relation_id: u64, level: u8, name: &str) {
  let conn = world().open_sqlite();
  let (stored_level, stored_name, wkb): (u8, String, Vec<u8>) = conn
    .query_row(SQL_SELECT_BOUNDARY, [relation_id], |r| {
      Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    })
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
  // the blob is spatialite's own layout, not iso wkb; decode it the way the source does.
  let geometry = SpatiaLiteWkb(wkb.as_slice())
    .to_geo()
    .unwrap_or_else(|e| panic!("relation {relation_id} ({name}) has an undecodable geometry: {e}"));
  assert!(
    matches!(geometry, Geometry::Polygon(_) | Geometry::MultiPolygon(_)),
    "relation {relation_id} ({name}) is not polygonal: its ring did not close"
  );
}

// 00.00. result quality
#[test]
#[ignore]
fn _00_00_a_house_number_on_the_street_resolves_as_exact() {
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

// 00.01. result quality
#[test]
#[ignore]
fn _00_01_a_house_number_between_two_known_ones_resolves_as_interpolated() {
  let w = world();
  let result = w.assert_cli(
    &ask("rua januario dos santos, santos 210"),
    &json!({ "matches": [{ "house_number": { "number": "210", "kind": "interpolated" } }] }),
  );
  assert!(levels_of(first(&result)).contains(&30));
}

// 00.02. result quality
#[test]
#[ignore]
fn _00_02_an_out_of_range_house_number_resolves_as_absent() {
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

// 00.03. result quality
#[test]
#[ignore]
fn _00_03_friendly_name_format_house_number_alias() {
  let w = world();
  w.assert_cli(
    &ask("rua januario dos santos, santos 197")
      .friendly_name_format("{admin_level_12_name} {house_number}"),
    &json!({ "matches": [{ "friendly_name": "Rua Januário dos Santos 197" }] }),
  );
}

// 00.04. result quality: the full payload for the documented text query, compared whole
#[test]
#[ignore]
fn _00_04_the_text_query_matches_are_exactly_these() {
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

// 00.05. result quality: the full top match for the documented coordinate, compared whole
#[test]
#[ignore]
fn _00_05_the_coordinate_query_top_match_is_exactly_this() {
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

// 00.06. result quality: the number appended on the coordinate path is the nearest stored point
// within 50 m of the query, measured to the number and not to the street
#[test]
#[ignore]
fn _00_06_a_house_number_within_50_m_of_the_point_is_appended_on_the_coordinate_path() {
  for (point, number) in [
    (NUMBERED_POINT, Some("197")),
    // 33 m from the street, 47 m from number 197
    ("-23.98160,-46.31005", Some("197")),
    // on the street, but 59 m from number 70 and 69 m from number 197
    ("-23.98158,-46.30958", None),
    // 1.8 m from 232, 6.7 m from 235, 25 m from 197
    ("-23.98226,-46.31031", Some("232")),
  ] {
    let result = world().run(&[point]);
    let top = first(&result);
    assert_eq!(
      name_at(top, 12).as_deref(),
      Some(NUMBERED_STREET_NAME),
      "point {point}"
    );
    assert_eq!(name_at(top, 30).as_deref(), number, "point {point}");
    if number.is_none() {
      assert_eq!(levels_of(top), [2, 4, 8, 10, 12], "point {point}");
    }
  }
}

// 00.07. result quality: the alias renders on the coordinate path, and a match without a number
// loses the literal that followed the placeholder
#[test]
#[ignore]
fn _00_07_the_house_number_alias_renders_on_the_coordinate_path() {
  world().assert_cli(
    &ask(NUMBERED_POINT)
      .friendly_name_format("{admin_level_12_name} {house_number}, {admin_level_8_name}"),
    &json!({
      "matches": [
        { "friendly_name": "Rua Januário dos Santos 197, Santos" },
        { "friendly_name": "Avenida Bartholomeu de Gusmão Santos" },
      ]
    }),
  );
}

// 01.00. precision guarantee
#[test]
#[ignore]
fn _01_00_min_quality_one_keeps_only_fully_covered_matches() {
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

// 01.01. precision guarantee
#[test]
#[ignore]
fn _01_01_bounding_wkt_keeps_only_matches_inside_the_polygon() {
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

// 01.02. precision guarantee
#[test]
#[ignore]
fn _01_02_a_bounding_polygon_over_open_water_excludes_everything() {
  let result = world().run(&[TEXT_QUERY, "--bounding-wkt", OUTSIDE_POLYGON]);
  assert!(
    matches(&result).is_empty(),
    "no street sits in that patch of water"
  );
}

// 01.03. precision guarantee
#[test]
#[ignore]
fn _01_03_last_admin_levels_keeps_only_the_requested_leaf() {
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

// 01.04. precision guarantee: the region restricts the candidates before the cut at ten, so the
// one street inside the polygon is answered even though it is not among the ten nearest
#[test]
#[ignore]
fn _01_04_bounding_wkt_keeps_a_coordinate_match_ranked_beyond_the_ten_nearest() {
  let w = world();
  let unbounded = w.run(&[NUMBERED_POINT]);
  let farthest = *distances(&unbounded)
    .last()
    .expect("the unbounded query answers ten streets");

  let far = w.run(&[NUMBERED_POINT, "--bounding-wkt", FAR_POLYGON]);
  assert!(
    !matches(&far).is_empty(),
    "the polygon holds at least one street"
  );
  for m in matches(&far) {
    let (lat, lon) = point_of(m);
    assert!(
      (-23.9830..=-23.9810).contains(&lat) && (-46.3040..=-46.3020).contains(&lon),
      "every match must lie inside the polygon: {}",
      m["friendly_name"]
    );
  }
  for distance in distances(&far) {
    assert!(
      distance > farthest,
      "the street inside the polygon is farther than the ten nearest"
    );
  }

  let nothing = w.run(&[NUMBERED_POINT, "--bounding-wkt", OUTSIDE_POLYGON]);
  assert!(
    matches(&nothing).is_empty(),
    "a polygon over open water holds no street"
  );
}

// 01.05. precision guarantee: a coordinate match's quality is 1 - distance / 100 m, and the
// matches come sorted by distance
#[test]
#[ignore]
fn _01_05_min_quality_on_coordinates_is_one_minus_the_distance_over_100_m() {
  let w = world();
  let unfiltered = distances(&w.run(&[NUMBERED_POINT]));
  assert!(
    unfiltered.windows(2).all(|pair| pair[0] <= pair[1]),
    "matches are sorted by distance: {unfiltered:?}"
  );
  // the second match sits at 64 m, a quality of 0.36
  for (min_quality, expected) in [("1", vec![0]), ("0.3", vec![0, 64]), ("0.4", vec![0])] {
    assert_eq!(
      distances(&w.run(&[NUMBERED_POINT, "--min-quality", min_quality])),
      expected,
      "--min-quality {min_quality}"
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

// 02.02. ambiguity
#[test]
#[ignore]
fn _02_02_the_surfaces_agree_under_every_flag() {
  let w = world();
  let s = w.start_server();
  let query = TEXT_QUERY;
  let any = json!({ "service": "text_to_address" });

  w.assert_both(
    &s,
    &ask(query).friendly_name_format("{admin_level_12_name} - {admin_level_8_name}"),
    &json!({ "matches": [{ "friendly_name": "Rua Castro Alves - Santos" }] }),
  );
  // a level the preset never extracts (6) must vanish without leaving its separator behind.
  w.assert_both(
    &s,
    &ask(query)
      .friendly_name_format("{admin_level_12_name}, {admin_level_6_name}, {admin_level_8_name}"),
    &json!({ "matches": [{ "friendly_name": "Rua Castro Alves, Santos" }] }),
  );
  w.assert_both(&s, &ask(query).min_quality("1.0"), &any);
  w.assert_both(&s, &ask(query).bounding_wkt(INSIDE_POLYGON), &any);
  w.assert_both(&s, &ask(query).last_admin_levels("12"), &any);
}

// 03.00. regression guard: a clipped boundary becomes a MultiLineString and never an ancestor
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
    .query_row(SQL_SELECT_COUNTRY_ISO_CODE, [], |r| r.get(0))
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

// 03.03. regression guard: the leaf filter runs at retrieval (12) and again after enrichment (30)
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

// 03.05. regression guard: on the coordinate path the leaf filter reads the level after the
// house-number step, so every candidate is kept until the end instead of being cut at ten early
#[test]
#[ignore]
fn _03_05_last_admin_levels_on_coordinates_reads_the_leaf_after_the_house_number() {
  let w = world();
  let ask_levels = |levels: &str| w.run(&[NUMBERED_POINT, "--last-admin-levels", levels]);

  let numbered = ask_levels("30");
  assert_eq!(leaves(&numbered), [30]);
  assert_eq!(name_at(first(&numbered), 30).as_deref(), Some("197"));

  let streets = ask_levels("12");
  assert_eq!(leaves(&streets), vec![12; 10]);
  assert_eq!(
    distances(&streets)[0],
    64,
    "the enriched street is dropped: its leaf is 30"
  );
  assert!(
    matches(&streets)
      .iter()
      .all(|m| name_at(m, 12).as_deref() != Some(NUMBERED_STREET_NAME)),
    "the numbered street has one segment and it ends at level 30"
  );

  assert_eq!(leaves(&ask_levels("12,30"))[0], 30);
  assert!(
    matches(&ask_levels("10")).is_empty(),
    "the coordinate service only answers streets"
  );

  let both = w.run(&[
    NUMBERED_POINT,
    "--bounding-wkt",
    APARECIDA_POLYGON,
    "--last-admin-levels",
    "30",
  ]);
  assert_eq!(leaves(&both), [30], "the filters apply as an and");
}

// 03.06. regression guard: the two services disagree on the street's own post code; the readme
// documents the divergence and the backlog item that unifies the rule flips this pin
#[test]
#[ignore]
fn _03_06_the_two_services_disagree_on_the_street_post_code() {
  for (input, post_code) in [
    (POST_CODED_STREET, Value::Null),
    (POST_CODED_POINT, json!("11320-060")),
  ] {
    world().assert_cli(
      &ask(input),
      &json!({
        "matches": [{
          "id": 1513188090,
          "friendly_name": "Ateneu São Vicente, São Paulo, Brasil, 11320-060",
          "attributes": { "post_code": post_code },
        }]
      }),
    );
  }
}

// 03.07. regression guard: within one level the coordinate path lists the ancestors from the
// general to the specific and the text path the other way round; the readme documents it
#[test]
#[ignore]
fn _03_07_the_two_services_order_same_level_ancestors_differently() {
  let w = world();
  for (input, level_10_names, rendered) in [
    (
      NESTED_POINT,
      ["Aparecida", "Conjunto Habitacional Jaú"],
      "Aparecida",
    ),
    (
      NESTED_QUERY,
      ["Conjunto Habitacional Jaú", "Aparecida"],
      "Conjunto Habitacional Jaú",
    ),
  ] {
    let result = w.assert_cli(&ask(input), &json!({ "matches": [{ "id": 371704170 }] }));
    let top = first(&result);
    assert_eq!(levels_of(top), [2, 4, 8, 10, 10, 12], "input {input:?}");
    assert_eq!(names_at(top, 10), level_10_names, "input {input:?}");

    w.assert_cli(
      &ask(input).friendly_name_format("{admin_level_10_name}"),
      &json!({ "matches": [{ "friendly_name": rendered }] }),
    );
  }
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
  let wkt_of =
    |level: u64| wkt_at(top, level).unwrap_or_else(|| panic!("level {level} must carry geometry"));
  assert!(
    wkt_of(12).starts_with("LINESTRING"),
    "streets are always lines"
  );

  let country = wkt_of(2);
  assert!(
    country.starts_with("MULTIPOLYGON"),
    "a closed country ring is a multipolygon"
  );
  assert!(
    country.len() > 1024 * 1024,
    "the brazil ring is megabytes; it is what makes the content-length contract testable"
  );
}

// 05.01. contract: the coordinate path loads the same geometry per level, and the level appended
// by the house-number step has none
#[test]
#[ignore]
fn _05_01_include_wkt_attaches_geometry_to_every_level_but_the_house_number_on_the_coordinate_path()
{
  // the quality cut leaves the one match at 0 m: every match would carry the country ring
  let result = world().query_json(&[NUMBERED_POINT, "--min-quality", "1"]);
  let top = first(&result);
  assert_eq!(levels_of(top), [2, 4, 8, 10, 12, 30]);
  assert!(
    wkt_at(top, 12).is_some_and(|wkt| wkt.starts_with("LINESTRING")),
    "streets are always lines"
  );
  for level in [10, 8, 4, 2] {
    assert!(
      wkt_at(top, level).is_some_and(|wkt| wkt.starts_with("MULTIPOLYGON")),
      "level {level} is an area"
    );
  }
  assert!(
    level_at(top, 30).is_some_and(|l| l.get("wkt").is_none()),
    "the house number is a point the service never loads"
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

// 02.03. ambiguity: the documented query plus ten spellings, every one landing on the same street
#[test]
#[ignore]
fn _02_03_every_spelling_of_the_address_lands_on_the_same_street() {
  let w = world();
  let s = w.start_server();
  for input in [
    TEXT_QUERY,
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

// 02.04. ambiguity: the documented coordinate plus ten points along the street, each landing on it
#[test]
#[ignore]
fn _02_04_every_point_along_the_street_lands_on_the_same_street() {
  let w = world();
  let s = w.start_server();
  for input in [
    COORDINATES_QUERY,
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

// 02.05. ambiguity: ten spellings of the same square, every one landing on the same square
#[test]
#[ignore]
fn _02_05_every_spelling_of_the_square_lands_on_the_same_square() {
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

// 02.06. ambiguity: ten points along the whole square, every one landing on the same square
#[test]
#[ignore]
fn _02_06_every_point_along_the_square_lands_on_the_same_square() {
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

// 02.07. ambiguity: ten spellings of the same numbered address, every one landing on the same house
#[test]
#[ignore]
fn _02_07_every_spelling_of_the_numbered_address_lands_on_the_same_house() {
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

// 02.08. ambiguity: five points along the whole street, every one landing on the same street
#[test]
#[ignore]
fn _02_08_every_point_along_the_numbered_street_lands_on_the_same_street() {
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

// 02.09. ambiguity: twelve spellings of the square address, every one landing on the same street
#[test]
#[ignore]
fn _02_09_every_spelling_of_the_square_address_lands_on_the_street_along_it() {
  let w = world();
  let s = w.start_server();
  for input in [
    "praca visconde de maua, 29 centro santos",
    "Praça Visconde de Mauá, 29, Centro, Santos",
    "PRACA VISCONDE DE MAUA 29 CENTRO SANTOS",
    "praca visconde de maua 29 - centro - santos",
    "pç. visconde de maua, 29, centro, santos",
    "pca. visconde de maua, 29, centro, santos",
    "praca visc. de maua, 29, centro, santos",
    "pç. visc. de maua, 29, centro, santos",
    "praca visconde de maua, nº 29, centro, santos",
    "29 praca visconde de maua centro santos",
    "santos, centro, praca visconde de maua, 29",
    "Praça Visconde de Mauá, 29 - Centro - Santos/SP",
  ] {
    let result = w.assert_both(
      &s,
      &ask(input),
      &json!({
        "service": "text_to_address",
        "matches": [{
          "friendly_name": "Rua Visconde de Mauá, Centro, Santos, São Paulo, Brasil",
          "house_number": { "number": "29", "kind": "absent" },
        }],
      }),
    );
    assert_eq!(
      levels_of(first(&result)),
      vec![2, 4, 8, 10, 12],
      "{input:?} must resolve the full ladder; an absent number never becomes a level"
    );
  }
}

// 02.10. ambiguity: five points along the street that borders the square, every one landing on it
#[test]
#[ignore]
fn _02_10_every_point_along_the_street_that_borders_the_square_lands_on_the_same_street() {
  let w = world();
  let s = w.start_server();
  for input in [
    "-23.933232,-46.329390",
    "-23.933388,-46.329408",
    "-23.933511,-46.329422",
    "-23.933634,-46.329436",
    "-23.933757,-46.329450",
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
            { "level": 10, "name": "Centro" },
            { "level": 12, "name": "Rua Visconde de Mauá" },
          ],
        }],
      }),
    );
  }
}

// 02.11. ambiguity: ten spellings of the same square, every one landing on the same square
#[test]
#[ignore]
fn _02_11_every_spelling_of_the_jose_menino_square_lands_on_the_same_square() {
  let w = world();
  let s = w.start_server();
  for input in [
    "praca washington jose menino santos",
    "Praça Washington, José Menino, Santos",
    "PRACA WASHINGTON JOSE MENINO SANTOS",
    "praca washington,jose menino,santos",
    "praca washington - jose menino - santos",
    "praca  washington   jose  menino  santos",
    "pç. washington jose menino santos",
    "pca. washington jose menino santos",
    "santos, jose menino, praca washington",
    "praça washington, josé menino, santos, são paulo",
  ] {
    let result = w.assert_both(
      &s,
      &ask(input),
      &json!({
        "service": "text_to_address",
        "matches": [{
          "friendly_name": "Praça Washington, José Menino, Santos, São Paulo, Brasil",
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

// 02.12. ambiguity: ten points around the whole square, every one landing on the same square
#[test]
#[ignore]
fn _02_12_every_point_around_the_jose_menino_square_lands_on_the_same_square() {
  let w = world();
  let s = w.start_server();
  for input in [
    "-23.967017,-46.350912",
    "-23.966730,-46.350136",
    "-23.966392,-46.349234",
    "-23.966175,-46.348913",
    "-23.965170,-46.348999",
    "-23.964902,-46.349362",
    "-23.964897,-46.349762",
    "-23.965344,-46.350957",
    "-23.966124,-46.353048",
    "-23.966073,-46.353005",
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
            { "level": 10, "name": "José Menino" },
            { "level": 12, "name": "Praça Washington" },
          ],
        }],
      }),
    );
  }
}
