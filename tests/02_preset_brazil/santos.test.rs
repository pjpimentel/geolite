use crate::common::ask::ask;
use crate::common::harness::{encode, get, scenario, world, world_cell};
use crate::common::query::{
  distances, first, kind_of, leaves, level_at, levels_of, matches, name_at, names_at, number_of,
  point_of, street_way, way_ids, wkt_at,
};
use crate::house_number::{
  CONSTANTS, FROM_OSM_DATA, MULTIPLE_REFERENCES, PLACEMENT_TOLERANCE_IN_METERS, assert_numbered,
};
use geo::{EuclideanDistance, Geometry, HaversineDistance, Point};
use geozero::{ToGeo, wkb::SpatiaLiteWkb, wkt::Wkt};
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

// the point of house number 197 on a single-segment street carrying the numbers 70, 197, 232, 235;
// the street ends 20 m past 235, where a number beyond every stored one lands
const NUMBERED_POINT: &str = "-23.98202,-46.31005";
const NUMBERED_STREET_NAME: &str = "Rua Januário dos Santos";
const NUMBERED_STREET_END: (f64, f64) = (-23.98243, -46.31049);
const APARECIDA_POLYGON: &str = "POLYGON((-46.3110 -23.9830,-46.3090 -23.9830,-46.3090 -23.9810,-46.3110 -23.9810,-46.3110 -23.9830))";
// ~700 m east of the numbered point: it holds one street, and that street is not among the ten
// nearest to the point
const FAR_POLYGON: &str = "POLYGON((-46.3040 -23.9830,-46.3020 -23.9830,-46.3020 -23.9810,-46.3040 -23.9810,-46.3040 -23.9830))";
const POST_CODED_STREET: &str = "Ateneu São Vicente";
const POST_CODED_POINT: &str = "-23.96675,-46.37675";
// rua deputado emilio justo carries its own post code and the numbers 23 and 259; the point is
// 23's, and the street ends 6 m past 259, where a number beyond every stored one lands
const POST_CODED_NUMBERED_QUERY: &str = "rua deputado emilio justo 23, 11725-440";
const POST_CODED_NUMBERED_POINT: &str = "-23.99429,-46.41588";
const POST_CODED_NUMBERED_STREET_END: (f64, f64) = (-23.99215, -46.41644);
// avenida washington luiz carries 565; the far point is on the avenue, beyond 50 m of every number
const NUMBERED_AVENUE_QUERY: &str = "avenida washington luiz 565, boqueirao";
const AVENUE_NUMBER_POINT: &str = "-23.96974,-46.32906";
const AVENUE_FAR_POINT: &str = "-23.96805,-46.32883";
// rua prefeito antenor bué lies inside conjunto habitacional jaú, inside aparecida, and carries 4
const NESTED_NUMBERED_QUERY: &str = "rua prefeito antenor bue 4";
const NESTED_NUMBERED_POINT: &str = "-23.97214,-46.30811";
// rua aureliano coutinho lies inside conjunto habitacional jaú, which lies inside aparecida: two
// ancestors of level 10
const NESTED_POINT: &str = "-23.973439,-46.309747";
const NESTED_QUERY: &str = "rua aureliano coutinho, conjunto habitacional jau, santos";
// avenida washington luiz meets avenida general francisco glicério here: two streets at 0 m
const TWO_AVENUES_POINT: &str = "-23.9574424,-46.327751";
// rua bento de abreu runs through boqueirão and embaré; three streets meet at its boqueirão point
const CROSSING_STREET_WAY: u64 = 255_734_641;
const CROSSING_STREET_IN_BOQUEIRAO: &str = "-23.966762,-46.322142";
const CROSSING_STREET_IN_EMBARE: &str = "-23.966895,-46.319555";
// rua castro alves carries 35 on the way that was folded into it; rua euclides da cunha is five ways
const FOLDED_NUMBERED_QUERY: &str = "rua castro alves, 35, embare";
const FIVE_WAYS_QUERY: &str = "rua euclides da cunha, gonzaga, santos, sao paulo";
const SUFFIXED_QUERY: &str = "rua inglaterra";
// three streets are named rua santos; only the one of guarujá carries 87
const NUMBERED_HOMONYM_QUERY: &str = "rua santos 87";
const NUMBERED_HOMONYM_WAY: u64 = 51_832_890;
// rua guarujá lies in saboó, which also holds a rua 11, and the ferries are named santos - guarujá
const TEMPTED_STREET_ANSWER: &str = "Rua Guarujá, 11, Saboó, Santos, São Paulo, Brasil";

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

fn post_coded_street_answer() -> Value {
  json!({
    "matches": [{
      "id": "c8e841fc-db3a-5bc8-ab40-019e7ef4b4d5",
      "friendly_name": "Ateneu São Vicente, São Paulo, Brasil, 11320-060",
      "admin_levels": [
        { "level": 2, "post_code": null },
        { "level": 4, "post_code": null },
        { "level": 12, "post_code": "11320-060" },
      ],
      "attributes": { "post_code": "11320-060" },
    }]
  })
}

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

fn assert_at_the_end_of_the_street(m: &Value, (latitude, longitude): (f64, f64)) {
  let (answered_latitude, answered_longitude) = point_of(m);
  let off = Point::new(answered_longitude, answered_latitude)
    .haversine_distance(&Point::new(longitude, latitude));
  assert!(
    off <= PLACEMENT_TOLERANCE_IN_METERS,
    "the point is {off:.1} m from the end of the street"
  );
}

// 00.00. result quality
#[test]
#[ignore]
fn _00_00_a_house_number_on_the_street_resolves_from_osm_data() {
  // rua januario dos santos is a single segment carrying 70, 197, 232 and 235 — no tie to break,
  // so the ranking is stable.
  let w = world();
  let result = w.assert_cli(
    &ask("rua januario dos santos, santos 197"),
    &json!({
      "matches": [{
        "friendly_name": "Rua Januário dos Santos, 197, Aparecida, Santos, São Paulo, Brasil",
        "house_number": { "number": "197", "kind": FROM_OSM_DATA },
      }],
    }),
  );
  assert_eq!(
    levels_of(first(&result)).last(),
    Some(&12),
    "the ladder ends at the street, the number is the object"
  );
}

// 00.01. result quality
#[test]
#[ignore]
fn _00_01_a_house_number_between_two_known_ones_is_presumed_from_the_references_of_the_street() {
  let w = world();
  w.assert_cli(
    &ask("rua januario dos santos, santos 210"),
    &json!({ "matches": [{ "house_number": { "number": "210", "kind": MULTIPLE_REFERENCES } }] }),
  );
}

// 00.02. result quality: a number beyond every stored one is presumed from them and stops at the
// end of the street its numbering grows to
#[test]
#[ignore]
fn _00_02_an_out_of_range_house_number_is_presumed_and_clamped_at_the_end_of_the_street() {
  let w = world();
  let numbered = w.assert_cli(
    &ask("rua januario dos santos, santos 99999"),
    &json!({ "matches": [{ "house_number": { "number": "99999", "kind": MULTIPLE_REFERENCES } }] }),
  );
  let top = first(&numbered);
  assert_eq!(
    levels_of(top).last().copied(),
    Some(12),
    "a presumed number is not a level"
  );
  assert_at_the_end_of_the_street(top, NUMBERED_STREET_END);
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
  // every match, whole and in order: the two ways of the street touch, so they answer as one
  // street, and the id of a match is the uuid of the path it took
  let result = world().run(&[TEXT_QUERY]);
  world().assert_exact(
    &Value::Array(matches(&result).clone()),
    &json!([
      {
        "admin_levels": [
          {
            "level": 2,
            "name": "Brasil",
            "post_code": null,
            "osm_relation_id": 59470,
            "osm_way_id": null,
          },
          {
            "level": 4,
            "name": "São Paulo",
            "post_code": null,
            "osm_relation_id": 298204,
            "osm_way_id": null,
          },
          {
            "level": 8,
            "name": "Santos",
            "post_code": null,
            "osm_relation_id": 298442,
            "osm_way_id": null,
          },
          {
            "level": 10,
            "name": "Embaré",
            "post_code": null,
            "osm_relation_id": 4282882,
            "osm_way_id": null,
          },
          {
            "level": 12,
            "name": "Rua Castro Alves",
            "post_code": null,
            "osm_relation_id": null,
            "osm_way_id": 255710390,
            "osm_merged_way_ids": [255710390, 729205713],
          },
        ],
        "attributes": {
          "country_iso_3166_1_alpha_2_code": "BR",
          "post_code": null,
        },
        "coordinates_distance_in_meters": null,
        "friendly_name": "Rua Castro Alves, Embaré, Santos, São Paulo, Brasil",
        "id": "667a5689-8c2e-50b0-a462-2fed26c98e82",
        "latitude": -23.9694,
        "longitude": -46.3175,
        "score": 108.702,
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
          "post_code": null,
          "osm_relation_id": 59470,
          "osm_way_id": null,
        },
        {
          "level": 4,
          "name": "São Paulo",
          "post_code": null,
          "osm_relation_id": 298204,
          "osm_way_id": null,
        },
        {
          "level": 8,
          "name": "Santos",
          "post_code": null,
          "osm_relation_id": 298442,
          "osm_way_id": null,
        },
        {
          "level": 10,
          "name": "Embaré",
          "post_code": null,
          "osm_relation_id": 4282882,
          "osm_way_id": null,
        },
        {
          "level": 12,
          "name": "Rua Castro Alves",
          "post_code": null,
          "osm_relation_id": null,
          "osm_way_id": 255710390,
          "osm_merged_way_ids": [255710390, 729205713],
        },
      ],
      "attributes": {
        "country_iso_3166_1_alpha_2_code": "BR",
        "post_code": null,
      },
      "coordinates_distance_in_meters": 3,
      "friendly_name": "Rua Castro Alves, 35, Embaré, Santos, São Paulo, Brasil",
      "house_number": {
        "number": "35",
        "kind": FROM_OSM_DATA,
        "osm_node_ids": [6_192_895_729_u64],
        "meters_per_number": null,
      },
      "id": "667a5689-8c2e-50b0-a462-2fed26c98e82",
      "latitude": -23.9709,
      "longitude": -46.3188,
      "score": null,
      "similarity": null,
    }),
  );
}

// 00.06. result quality: a point reads the stored number within 50 m of it, measured to the
// number and not to the street, and beyond 50 m a number presumed from the stored ones
#[test]
#[ignore]
fn _00_06_a_point_reads_the_stored_number_within_fifty_metres_and_a_presumed_one_beyond() {
  for (point, numbers, kind) in [
    (NUMBERED_POINT, &["197"][..], FROM_OSM_DATA),
    // 33 m from the street, 47 m from number 197
    ("-23.98160,-46.31005", &["197"][..], FROM_OSM_DATA),
    // on the street, 59 m from number 70 and 69 m from number 197: read between the two
    (
      "-23.98158,-46.30958",
      &["128", "129"][..],
      MULTIPLE_REFERENCES,
    ),
    // 1.9 m from 232, 6.5 m from 235, 37 m from 197
    ("-23.98226,-46.31031", &["232"][..], FROM_OSM_DATA),
  ] {
    let result = world().run(&[point]);
    let top = first(&result);
    assert_eq!(
      name_at(top, 12).as_deref(),
      Some(NUMBERED_STREET_NAME),
      "point {point}"
    );
    let number = number_of(top).unwrap_or_else(|| panic!("point {point} answers no number"));
    assert!(numbers.contains(&number), "point {point}: {number}");
    assert_eq!(kind_of(top), Some(kind), "point {point}");
  }
}

// 00.07. result quality: the alias renders on the coordinate path, for a stored number and for a
// presumed one, and a match without a number loses the literal that followed the placeholder
#[test]
#[ignore]
fn _00_07_the_house_number_alias_renders_on_the_coordinate_path() {
  const TEMPLATE: &str = "{admin_level_12_name} {house_number}, {admin_level_8_name}";

  let result = world().assert_cli(
    &ask(NUMBERED_POINT).friendly_name_format(TEMPLATE),
    &json!({ "matches": [{ "friendly_name": "Rua Januário dos Santos 197, Santos" }] }),
  );
  // the avenue is a folded street of twenty lines: its presumed number is read back, not pinned
  let second = &matches(&result)[1];
  let number = number_of(second).expect("every street a point answers carries a number");
  assert_eq!(
    second["friendly_name"],
    format!("Avenida Bartholomeu de Gusmão {number}, Santos")
  );
  world().assert_cli(
    &ask("rua bolivar, boqueirao, santos").friendly_name_format(TEMPLATE),
    &json!({ "matches": [{ "friendly_name": "Rua Bolivar Santos" }] }),
  );
}

// 00.08. result quality: the label with a house number follows the same rule as the label
// without one — the names outward, the number after the street, the post codes at the end
#[test]
#[ignore]
fn _00_08_a_house_number_label_keeps_the_post_code_on_both_paths() {
  for input in [POST_CODED_NUMBERED_QUERY, POST_CODED_NUMBERED_POINT] {
    world().assert_cli(
      &ask(input),
      &json!({
        "matches": [{
          "friendly_name": "Rua Deputado Emilio Justo, 23, Sítio do Campo, São Paulo, Brasil, 11725-440",
        }]
      }),
    );
  }
}

// 00.09. result quality: the numbered label follows the path, not the ladder, so both services
// write it the same way even where the coordinate ladder lists a level from the general side
#[test]
#[ignore]
fn _00_09_both_services_write_the_numbered_label_along_the_path() {
  for input in [NESTED_NUMBERED_QUERY, NESTED_NUMBERED_POINT] {
    world().assert_cli(
      &ask(input),
      &json!({
        "matches": [{
          "id": "e62f6c36-4709-566a-a4d8-be7167c931c6",
          "friendly_name": "Rua Prefeito Antenor Bué, 4, Conjunto Habitacional Jaú, Aparecida, Santos, São Paulo, Brasil",
        }]
      }),
    );
  }
}

// 00.10. result quality: a number beyond every stored one joins the label like a stored one, and
// lands at the end of the street its numbering grows to
#[test]
#[ignore]
fn _00_10_an_out_of_range_number_joins_the_label_and_stops_at_the_end_the_numbering_grows_towards()
{
  let result = world().assert_cli(
    &ask("rua deputado emilio justo 99999, 11725-440"),
    &json!({
      "matches": [{
        "friendly_name": "Rua Deputado Emilio Justo, 99999, Sítio do Campo, São Paulo, Brasil, 11725-440",
        "house_number": { "number": "99999", "kind": MULTIPLE_REFERENCES },
      }]
    }),
  );
  let top = first(&result);
  assert_eq!(levels_of(top), [2, 4, 10, 12]);
  assert_at_the_end_of_the_street(top, POST_CODED_NUMBERED_STREET_END);
}

// 00.11. result quality: a presumed number joins the label like a stored one, before the post
// code
#[test]
#[ignore]
fn _00_11_a_presumed_number_keeps_the_post_code_at_the_end() {
  let result = world().assert_cli(
    &ask("rua deputado emilio justo 100, 11725-440"),
    &json!({
      "matches": [{
        "friendly_name": "Rua Deputado Emilio Justo, 100, Sítio do Campo, São Paulo, Brasil, 11725-440",
        "house_number": { "number": "100", "kind": MULTIPLE_REFERENCES },
      }]
    }),
  );
  assert_eq!(levels_of(first(&result)).last(), Some(&12));
}

// 00.12. result quality: a template reads the ladder and the number, and never the post codes the
// default label ends with
#[test]
#[ignore]
fn _00_12_the_template_renders_the_house_number_without_the_post_code() {
  world().assert_cli(
    &ask(POST_CODED_NUMBERED_QUERY).friendly_name_format("{admin_level_12_name}, {house_number}"),
    &json!({ "matches": [{ "friendly_name": "Rua Deputado Emilio Justo, 23" }] }),
  );
}

// 00.13. result quality: a stored number is read only within 50 m of the point; beyond it the same
// street answers a number presumed from its stored ones
#[test]
#[ignore]
fn _00_13_a_point_beyond_fifty_metres_of_every_stored_number_answers_a_presumed_one() {
  let w = world();
  let near = first(&w.run(&[AVENUE_NUMBER_POINT])).clone();
  assert_eq!(
    name_at(&near, 12).as_deref(),
    Some("Avenida Washington Luiz")
  );
  assert_numbered(&near, "565", FROM_OSM_DATA, "the point beside 565");

  let at_far_point = w.run(&[AVENUE_FAR_POINT]);
  let far = matches(&at_far_point)
    .iter()
    .find(|m| name_at(m, 12).as_deref() == Some("Avenida Washington Luiz"))
    .expect("the avenue is among the streets at the point");
  assert_eq!(
    far["coordinates_distance_in_meters"], 0,
    "the point is on the avenue"
  );
  // the avenue carries 361 and 565 on two of its fourteen lines, none within 50 m of the point;
  // the axis joins the two lines through lines that touch end to end and keep their heading
  assert_eq!(kind_of(far), Some(MULTIPLE_REFERENCES));
  assert_eq!(levels_of(far), [2, 4, 8, 10, 12]);
}

// 00.14. result quality: the text path and the coordinate path read the same stored numbers
#[test]
#[ignore]
fn _00_14_both_paths_read_the_same_stored_number() {
  let w = world();
  w.assert_cli(
    &ask(NUMBERED_AVENUE_QUERY),
    &json!({
      "matches": [{
        "friendly_name": "Avenida Washington Luiz, 565, Boqueirão, Santos, São Paulo, Brasil",
        "house_number": { "number": "565", "kind": FROM_OSM_DATA },
      }]
    }),
  );
  assert_eq!(
    number_of(first(&w.run(&[AVENUE_NUMBER_POINT]))),
    Some("565")
  );
}

// 00.15. result quality: a number that sat on one way of a street resolves on the street its ways
// were folded into, and the street answers once
#[test]
#[ignore]
fn _00_15_a_number_on_one_way_of_a_folded_street_resolves_from_osm_data_on_the_one_match() {
  let w = world();
  let result = w.assert_cli(
    &ask("rua castro alves, 35, embare"),
    &json!({
      "matches": [{
        "friendly_name": "Rua Castro Alves, 35, Embaré, Santos, São Paulo, Brasil",
        "house_number": { "number": "35", "kind": FROM_OSM_DATA },
      }],
    }),
  );
  let in_embare = matches(&result)
    .iter()
    .filter(|m| name_at(m, 10).as_deref() == Some("Embaré"))
    .count();
  assert_eq!(in_embare, 1, "the two ways of the street answer as one");
}

// 00.16. result quality: a street of several ways answers with a point on the street, not on the
// mean of its ways
#[test]
#[ignore]
fn _00_16_a_street_of_several_ways_answers_a_point_on_the_street() {
  let result = world().query_json(&[TEXT_QUERY]);
  let top = first(&result);
  let street = Wkt(
    wkt_at(top, 12)
      .expect("the street carries geometry")
      .as_str(),
  )
  .to_geo()
  .expect("the street geometry must be valid wkt");
  let (latitude, longitude) = point_of(top);
  let distance = Point::new(longitude, latitude).euclidean_distance(&street);
  assert!(
    distance < 0.00002,
    "the point is {distance} degrees away from the street"
  );
}

// 00.17. result quality: a suffixed number matches its stored form whatever the case typed
#[test]
#[ignore]
fn _00_17_a_suffixed_number_matches_its_stored_form_whatever_the_case_typed() {
  for typed in ["40a", "40A"] {
    let result = world().run(&[&format!("{SUFFIXED_QUERY} {typed}")]);
    assert_eq!(
      name_at(first(&result), 12).as_deref(),
      Some("Rua Inglaterra"),
      "{typed}: the street ranks first"
    );
    assert_numbered(
      first(&result),
      typed,
      FROM_OSM_DATA,
      &format!("{typed}: the fixture stores 40A on Rua Inglaterra"),
    );
  }
}

// 00.18. result quality: a typed number is a word no document holds, and the street covering every
// other word still ranks first; the fixture tempts with a rua 11 in saboó and with the ferries
// named santos - guarujá
#[test]
#[ignore]
fn _00_18_a_street_covering_every_word_but_the_number_ranks_first() {
  let result = world().run(&["rua guaruja 11 saboo santos"]);
  assert_eq!(first(&result)["friendly_name"], TEMPTED_STREET_ANSWER);
}

// 00.19. result quality: the number is demanded of no document, so where it is typed does not
// change the street that ranks first, on either surface
#[test]
#[ignore]
fn _00_19_the_place_of_the_number_in_the_text_does_not_change_the_top_match() {
  let w = world();
  let s = w.start_server();
  for typed in [
    "11 rua guaruja saboo santos",
    "rua guaruja 11 saboo santos",
    "rua guaruja, 11, saboo, santos",
    "rua guaruja saboo santos 11",
  ] {
    w.assert_both(
      &s,
      &ask(typed),
      &json!({
        "matches": [{
          "friendly_name": TEMPTED_STREET_ANSWER,
          "house_number": { "number": "11" },
        }],
      }),
    );
  }
}

// 00.20. result quality: with two numbers typed neither is demanded of a document, and the first
// one is the house number
#[test]
#[ignore]
fn _00_20_the_first_of_two_typed_numbers_is_the_house_number() {
  let result = world().run(&["rua guaruja 11 22 saboo santos"]);
  assert_eq!(first(&result)["friendly_name"], TEMPTED_STREET_ANSWER);
  assert_eq!(number_of(first(&result)), Some("11"));
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

// 01.06. precision guarantee: a multipolygon keeps what its polygons keep, here the inside one
// joined with one over open water
#[test]
#[ignore]
fn _01_06_a_multipolygon_bounding_keeps_what_its_polygons_keep() {
  let multipolygon = format!(
    "MULTIPOLYGON({},{})",
    INSIDE_POLYGON.trim_start_matches("POLYGON"),
    OUTSIDE_POLYGON.trim_start_matches("POLYGON")
  );
  let joined = world().run(&[TEXT_QUERY, "--bounding-wkt", &multipolygon]);
  assert!(
    !matches(&joined).is_empty(),
    "the inside polygon holds the street"
  );
  assert_eq!(
    way_ids(&joined),
    way_ids(&world().run(&[TEXT_QUERY, "--bounding-wkt", INSIDE_POLYGON])),
  );
}

// 01.07. precision guarantee: the similarity counts the query's tokens against the text of each
// document, so a name that exists elsewhere in the database covers nothing here
#[test]
#[ignore]
fn _01_07_min_quality_one_reads_the_coverage_of_each_document() {
  let w = world();
  for (query, answers) in [
    ("rua castro alves, embare, santos", true),
    ("rua castro alves, embare, guaruja", false),
    ("rua castro alves, embare, santos, sao paulo, xyzzy", false),
  ] {
    assert_eq!(
      !matches(&w.run(&[query, "--min-quality", "1.0"])).is_empty(),
      answers,
      "query {query:?}"
    );
  }
}

// 01.08. precision guarantee: the document's text carries the post code in both forms, so the
// digits-only one covers it whole
#[test]
#[ignore]
fn _01_08_a_digits_only_post_code_covers_the_document() {
  let result = world().run(&["rua deputado emilio justo 11725440", "--min-quality", "1.0"]);
  assert_eq!(
    name_at(first(&result), 12).as_deref(),
    Some("Rua Deputado Emilio Justo"),
  );
}

// 01.09. precision guarantee: the region holds while the number is not demanded: the polygon
// around the street answers it with its number, and the one beside it never answers it
#[test]
#[ignore]
fn _01_09_bounding_wkt_keeps_a_numbered_match_inside_the_polygon_and_drops_it_outside() {
  let query = "rua januario dos santos, santos 197";
  let around = world().run(&[query, "--bounding-wkt", APARECIDA_POLYGON]);
  assert_eq!(
    name_at(first(&around), 12).as_deref(),
    Some(NUMBERED_STREET_NAME)
  );
  assert_eq!(kind_of(first(&around)), Some(FROM_OSM_DATA));

  let beside = world().run(&[query, "--bounding-wkt", INSIDE_POLYGON]);
  assert!(
    matches(&beside)
      .iter()
      .all(|m| name_at(m, 12).as_deref() != Some(NUMBERED_STREET_NAME)),
    "the street lies outside the polygon"
  );
}

// 01.10. precision guarantee: the region reads the point of the number and not the one of the
// street: 197 lies inside the polygon, and 1 lands at the end of the street that is outside it
#[test]
#[ignore]
fn _01_10_a_region_keeps_a_numbered_match_by_the_point_of_the_number() {
  let answered = |number: &str| {
    let query = format!("rua januario dos santos, santos {number}");
    let result = world().run(&[&query, "--bounding-wkt", APARECIDA_POLYGON]);
    matches(&result)
      .iter()
      .any(|m| name_at(m, 12).as_deref() == Some(NUMBERED_STREET_NAME))
  };
  assert!(answered("197"), "197 is inside the polygon");
  assert!(!answered("1"), "1 is placed outside the polygon");
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

// 03.03. regression guard: the leaf of a numbered street is the street, so `12` keeps the match
// whose number came from the osm data, and `30` is no level: both services refuse it where the
// levels are parsed
#[test]
#[ignore]
fn _03_03_a_numbered_street_keeps_its_leaf_at_the_street_and_level_30_is_refused() {
  let w = world();
  let query = NUMBERED_HOMONYM_QUERY;

  let streets = w.run(&[query, "--last-admin-levels", "12"]);
  let top = first(&streets);
  assert_eq!(
    kind_of(top),
    Some(FROM_OSM_DATA),
    "the street that stores 87 ranks first"
  );
  assert_eq!(street_way(top), Some(NUMBERED_HOMONYM_WAY));
  assert!(
    leaves(&streets).iter().all(|&leaf| leaf == 12),
    "the number never enters the ladder: every leaf is the street"
  );

  let out = w.geolite(&["query", query, "--last-admin-levels", "30"]);
  assert_eq!(out.status, 2);
  assert!(
    out.stderr.contains("level 30 is not supported"),
    "stderr: {}",
    out.stderr
  );

  let s = w.start_server();
  let r = get(s.port, &ask(query).last_admin_levels("30").http_path());
  assert_eq!(r.status, 400, "body: {}", r.text());
  assert_eq!(
    r.json()["error"],
    "last_admin_levels: level 30 is not supported"
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

// 03.05. regression guard: on the coordinate path the leaf filter runs on the candidates, whose
// leaf is the street itself, so the cut at ten comes before the loads and `12` keeps the numbered
// street at 0 m first
#[test]
#[ignore]
fn _03_05_last_admin_levels_on_coordinates_keeps_the_numbered_street_under_the_street_level() {
  let w = world();
  let ask_levels = |levels: &str| w.run(&[NUMBERED_POINT, "--last-admin-levels", levels]);

  let streets = ask_levels("12");
  assert_eq!(
    leaves(&streets),
    vec![12; 10],
    "every street a point answers ends at the street"
  );
  let top = first(&streets);
  assert_eq!(name_at(top, 12).as_deref(), Some(NUMBERED_STREET_NAME));
  assert_eq!(number_of(top), Some("197"));
  assert_eq!(kind_of(top), Some(FROM_OSM_DATA));
  assert_eq!(
    distances(&streets)[0],
    0,
    "the numbered street is kept, number and all"
  );
  assert!(
    matches(&ask_levels("10")).is_empty(),
    "the coordinate service only answers streets"
  );

  let both = w.run(&[
    NUMBERED_POINT,
    "--bounding-wkt",
    APARECIDA_POLYGON,
    "--last-admin-levels",
    "12",
  ]);
  assert!(
    leaves(&both).iter().all(|&leaf| leaf == 12),
    "the filters apply as an and"
  );
  assert!(
    matches(&both).len() < matches(&streets).len(),
    "the polygon keeps fewer than the ten streets"
  );
  assert_eq!(
    name_at(first(&both), 12).as_deref(),
    Some(NUMBERED_STREET_NAME)
  );
}

// 03.06. regression guard: every level answers its own post code, and the attributes the most
// specific of the path, on both services
#[test]
#[ignore]
fn _03_06_both_services_answer_the_street_post_code() {
  let w = world();
  w.assert_cli(&ask(POST_CODED_STREET), &post_coded_street_answer());

  // the point carries a presumed number in its label; every post code stays where it was
  let mut expected = post_coded_street_answer();
  expected["matches"][0]
    .as_object_mut()
    .expect("a match is an object")
    .remove("friendly_name");
  let result = w.assert_cli(&ask(POST_CODED_POINT), &expected);
  let top = first(&result);
  let number = number_of(top).expect("every street a point answers carries a number");
  assert_eq!(
    top["friendly_name"],
    format!("Ateneu São Vicente, {number}, São Paulo, Brasil, 11320-060")
  );
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
    let result = w.assert_cli(
      &ask(input),
      &json!({ "matches": [{ "id": "9695f186-46eb-539f-a831-ac0489ebd841" }] }),
    );
    let top = first(&result);
    assert_eq!(levels_of(top), [2, 4, 8, 10, 10, 12], "input {input:?}");
    assert_eq!(names_at(top, 10), level_10_names, "input {input:?}");

    w.assert_cli(
      &ask(input).friendly_name_format("{admin_level_10_name}"),
      &json!({ "matches": [{ "friendly_name": rendered }] }),
    );
  }
}

// 03.08. regression guard: the http api answers the post code of every level and of the
// attributes exactly as the cli does
#[test]
#[ignore]
fn _03_08_the_http_api_answers_the_post_code_of_every_level() {
  let w = world();
  let s = w.start_server();
  w.assert_both(&s, &ask(POST_CODED_STREET), &post_coded_street_answer());
}

// 03.09. regression guard: the distance is whole metres, so the streets that meet at a point tie,
// and the id breaks the tie: the order of the answer never depends on the machine
#[test]
#[ignore]
fn _03_09_streets_at_the_same_distance_rank_by_id() {
  for (point, ways) in [
    (AVENUE_FAR_POINT, vec![32_338_918, 38_791_115]),
    (TWO_AVENUES_POINT, vec![32_338_918, 38_794_373]),
    (
      CROSSING_STREET_IN_BOQUEIRAO,
      vec![38_794_576, CROSSING_STREET_WAY, 502_795_198],
    ),
  ] {
    let result = world().run(&[point]);
    let on_the_point: Vec<u64> = matches(&result)
      .iter()
      .filter(|m| m["coordinates_distance_in_meters"] == 0)
      .filter_map(street_way)
      .collect();
    assert_eq!(on_the_point, ways, "the streets at 0 m of {point}");
  }
}

// 03.10. regression guard: a street across two neighbourhoods answers a point once, under the
// neighbourhood that holds the point of the street nearest to it, and not once per neighbourhood
#[test]
#[ignore]
fn _03_10_a_point_answers_a_street_across_two_neighbourhoods_under_the_one_that_holds_it() {
  for (point, neighbourhood) in [
    (CROSSING_STREET_IN_BOQUEIRAO, "Boqueirão"),
    (CROSSING_STREET_IN_EMBARE, "Embaré"),
  ] {
    let result = world().run(&[point]);
    let answers: Vec<&Value> = matches(&result)
      .iter()
      .filter(|m| street_way(m) == Some(CROSSING_STREET_WAY))
      .collect();
    assert_eq!(answers.len(), 1, "the street answers {point} once");
    assert_eq!(names_at(answers[0], 10), [neighbourhood]);
  }
}

// 03.11. regression guard: the http api names the ways of a folded street exactly as the cli does,
// and neither names them on a level that is not a fold
#[test]
#[ignore]
fn _03_11_the_http_api_names_the_ways_of_a_folded_street_and_of_no_other_level() {
  let w = world();
  let s = w.start_server();
  let result = w.assert_both(
    &s,
    &ask(FOLDED_NUMBERED_QUERY),
    &json!({ "matches": [{ "house_number": { "number": "35", "kind": FROM_OSM_DATA } }] }),
  );
  let top = first(&result);
  assert_eq!(
    level_at(top, 12).map(|street| &street["osm_merged_way_ids"]),
    Some(&json!([255_710_390_u64, 729_205_713_u64]))
  );
  for level in levels_of(top).into_iter().filter(|level| *level != 12) {
    assert!(
      level_at(top, level).is_some_and(|l| l.get("osm_merged_way_ids").is_none()),
      "level {level} is not a fold"
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
    wkt_of(12).starts_with("MULTILINESTRING"),
    "the two ways of the street fold into a multi-line"
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

// 05.01. contract: the coordinate path loads the same geometry per level
#[test]
#[ignore]
fn _05_01_include_wkt_attaches_geometry_to_every_level_on_the_coordinate_path() {
  // the quality cut leaves the one match at 0 m: every match would carry the country ring
  let result = world().query_json(&[NUMBERED_POINT, "--min-quality", "1"]);
  let top = first(&result);
  assert_eq!(levels_of(top), [2, 4, 8, 10, 12]);
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
}

// 05.02. contract: the wkt of a folded street and the ways it names describe each other: one line
// per way, the street's own way first
#[test]
#[ignore]
fn _05_02_the_wkt_of_a_folded_street_has_one_line_per_merged_way() {
  for (query, ways) in [(TEXT_QUERY, 2), (FIVE_WAYS_QUERY, 5)] {
    let result = world().query_json(&[query]);
    let street = level_at(first(&result), 12).expect("the top match is a street");
    let trace = street["osm_merged_way_ids"]
      .as_array()
      .unwrap_or_else(|| panic!("{query:?} answers a folded street"));
    assert_eq!(trace.len(), ways, "the fixture changed for {query:?}");
    assert_eq!(trace[0], street["osm_way_id"], "its own way comes first");
    let wkt = street["wkt"].as_str().expect("the street carries geometry");
    match Wkt(wkt)
      .to_geo()
      .expect("the street geometry must be valid wkt")
    {
      Geometry::MultiLineString(lines) => assert_eq!(lines.0.len(), trace.len()),
      other => panic!("{query:?} answers a street that is not a multi-line: {other:?}"),
    }
  }
}

// 05.03. contract: a last placeholder that is missing leaves no separator behind, and an empty
// format names nothing
#[test]
#[ignore]
fn _05_03_a_missing_last_placeholder_leaves_no_separator_and_an_empty_format_names_nothing() {
  let w = world();
  let s = w.start_server();
  w.assert_both(
    &s,
    &ask(TEXT_QUERY).friendly_name_format("{admin_level_12_name}, {admin_level_6_name}"),
    &json!({ "matches": [{ "friendly_name": "Rua Castro Alves" }] }),
  );
  w.assert_both(
    &s,
    &ask(TEXT_QUERY).friendly_name_format(""),
    &json!({ "matches": [{ "friendly_name": "" }] }),
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

// 06.01. dead case: a typo sends the whole text to the loose query, and the number typed beside
// it is still read from the street that stores it
#[test]
#[ignore]
fn _06_01_a_typo_beside_a_number_still_resolves_the_number() {
  let result = world().run(&["rua castro alvez, 35, embare"]);
  let street = matches(&result)
    .iter()
    .find(|m| name_at(m, 12).as_deref() == Some("Rua Castro Alves"))
    .expect("the fuzzy fallback must still find the street");
  assert_numbered(street, "35", FROM_OSM_DATA, "35 beside a typo");
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
      vec![2, 4, 8, 10, 12],
      "{input:?} must resolve the full ladder down to the street"
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

// 02.09. ambiguity: twelve spellings of the square address, every one landing on the same street,
// with the number presumed from the preset's metres per number on a street without any
#[test]
#[ignore]
fn _02_09_every_spelling_of_the_square_address_lands_on_the_street_along_it_with_a_presumed_number()
{
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
          "friendly_name": "Rua Visconde de Mauá, 29, Centro, Santos, São Paulo, Brasil",
          "house_number": { "number": "29", "kind": CONSTANTS },
        }],
      }),
    );
    assert_eq!(
      levels_of(first(&result)),
      vec![2, 4, 8, 10, 12],
      "{input:?} must resolve the full ladder down to the street, the number presumed beside it"
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

// 02.12. ambiguity: ten points around the whole square, every one landing on the same square; the
// square hangs from José Menino and from Marapé, and a point answers the neighbourhood that holds
// the point of the square nearest to it
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
    let result = w.assert_both(
      &s,
      &ask(input),
      &json!({ "service": "coordinates_to_address" }),
    );
    let top = first(&result);
    assert_eq!(
      name_at(top, 12).as_deref(),
      Some("Praça Washington"),
      "{input}"
    );
    assert!(
      matches!(name_at(top, 10).as_deref(), Some("José Menino" | "Marapé")),
      "{input}: the square hangs from both neighbourhoods"
    );
  }
}
