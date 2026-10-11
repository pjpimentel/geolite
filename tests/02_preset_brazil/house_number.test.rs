use crate::common::ask::ask;
use crate::common::harness::{decode_wkb, query_at};
use crate::common::query::{
  distances, first, kind_of, levels_of, matches, meters_per_number, name_at, number_of,
  osm_node_ids, point_of, street_way, wkt_at,
};
use crate::hierarchy::{REGENERATE, geometry_of, way};
use crate::santos::world;
use geo::{
  EuclideanDistance, Geometry, HaversineDistance, HaversineLength, LineInterpolatePoint,
  LineLocatePoint, LineString, Point,
};
use geozero::{ToGeo, wkt::Wkt};
use serde_json::{Value, json};

pub(crate) const FROM_OSM_DATA: &str = "from_osm_data";
pub(crate) const MULTIPLE_REFERENCES: &str = "presumed_from_multiple_references_from_street";
pub(crate) const ONE_REFERENCE: &str = "presumed_from_one_ref_from_street";
pub(crate) const CONSTANTS: &str = "presumed_from_constants";
const SCENARIOS: [&str; 4] = [FROM_OSM_DATA, MULTIPLE_REFERENCES, ONE_REFERENCE, CONSTANTS];

// the metres one number takes: one under `brazil`, as the fixture measures, three by default
const BRAZIL_METERS_PER_NUMBER: f64 = 1.0;
const DEFAULT_METERS_PER_NUMBER: f64 = 3.0;

// rua januário dos santos is a straight line of two points, 265 m long, carrying 70, 197, 232 and
// 235 at 73, 201, 236 and 245 m from its start as drawn
const NUMBERED_STREET: &str = "Rua Januário dos Santos";
const NUMBERED_STREET_ID: i64 = 256_305_358;
const NUMBERED_STREET_QUERY: &str = "rua januario dos santos, santos";
// the point of 197, and a point 33 m off the street that is 47 m from 197
const NUMBERED_POINT: (f64, f64) = (-23.98202, -46.31005);
const NEAR_NUMBERED_POINT: (f64, f64) = (-23.98160, -46.31005);
// rua santos in guarujá, 466 m in seven points, carries 51, 87, 92, 141, 176, 200 and 208 between
// 400 and 244 m from its start as drawn: its numbering grows towards the start, a metre a number
const MANY_REFERENCES_STREET: &str = "Rua Santos";
const MANY_REFERENCES_STREET_ID: i64 = 103_665_780;
// rua governador fernando costa, 639 m in twelve points, carries only 343, 304 m from its start as
// drawn: 343 reads as metres from the other end, so its numbering grows towards the start
const ONE_REFERENCE_STREET: &str = "Rua Governador Fernando Costa";
const ONE_REFERENCE_STREET_ID: i64 = 957_013_978;
const ONE_REFERENCE_STREET_QUERY: &str = "rua governador fernando costa, ponta da praia, santos";
const ONE_REFERENCE_NUMBER: u32 = 343;
// the neighbourhood of that street ranks among its streets when the query names nothing else; 410
// is a word of no document, which 400 is, of the post code 11030-400
const NEIGHBOURHOOD_IN_THE_QUERY: &str = "Ponta da Praia";
const NEIGHBOURHOOD_QUERY: &str = "ponta da praia, santos";
const NEIGHBOURHOOD_NUMBER: &str = "410";
// rua bolivar, 314 m in three points, carries no number
const BARE_STREET: &str = "Rua Bolivar";
const BARE_STREET_ID: i64 = 371_699_630;
const BARE_STREET_QUERY: &str = "rua bolivar, boqueirao, santos";
// the digits of these two names are words of the name; the rua 7 de setembro of vila nova, way
// 48459786, carries 47 and outranks its homonym without numbers
const NAMED_BY_NUMBER_STREET: &str = "Rua 15 de Novembro";
const NAMED_BY_NUMBER_QUERY: &str = "rua 15 de novembro, santos";
const NAMED_AND_NUMBERED_STREET: &str = "Rua 7 de Setembro";
const NAMED_AND_NUMBERED_QUERY: &str = "rua 7 de setembro, santos 100";
const NAMED_AND_NUMBERED_WAY: u64 = 48_459_786;
// rua bento de abreu runs through boqueirão and embaré, and carries one number
const CROSSING_STREET: &str = "Rua Bento de Abreu";
const CROSSING_STREET_QUERY: &str = "rua bento de abreu";
// rua inglaterra carries 38, 40 and 40A
const SUFFIXED_STREET: &str = "Rua Inglaterra";
const SUFFIXED_STREET_QUERY: &str = "rua inglaterra";

// the response rounds a point to five decimals, which moves it by up to a metre
pub(crate) const PLACEMENT_TOLERANCE_IN_METERS: f64 = 2.0;
const NUMBER_TOLERANCE: f64 = 1.0;
const ON_STREET_TOLERANCE_IN_DEGREES: f64 = 0.00002;

fn street_line(id: i64) -> LineString<f64> {
  match geometry_of(&world().open_sqlite(), id) {
    Geometry::LineString(line) => line,
    _ => panic!("street {id} is not one line; {REGENERATE}"),
  }
}

// the stored numbers of a street as (node, value, point), one per value, the first in node id
// order
fn stored_references_of(id: i64) -> Vec<(u64, u32, Point<f64>)> {
  const SQL_SELECT_STORED_NUMBERS: &str = "
    SELECT node_id, number, wkb
    FROM house_numbers
    WHERE admin_level_id = ?1
    ORDER BY node_id
  ";

  let conn = world().open_sqlite();
  let mut stmt = conn
    .prepare(SQL_SELECT_STORED_NUMBERS)
    .expect("failed to prepare the stored numbers");
  let rows: Vec<(u64, String, Vec<u8>)> = stmt
    .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
    .expect("failed to query the stored numbers")
    .map(|r| r.expect("failed to read a stored number"))
    .collect();
  let mut references: Vec<(u64, u32, Point<f64>)> = Vec::new();
  for (node, number, blob) in &rows {
    let value: u32 = number
      .parse()
      .unwrap_or_else(|_| panic!("{number} is not a plain number; {REGENERATE}"));
    let Geometry::Point(point) = decode_wkb(blob) else {
      panic!("the stored number {number} is not a point; {REGENERATE}")
    };
    if !references.iter().any(|(_, stored, _)| *stored == value) {
      references.push((*node, value, point));
    }
  }
  references
}

// the stored numbers of a street as (value, metres along its line)
fn references_of(id: i64, line: &LineString<f64>) -> Vec<(u32, f64)> {
  stored_references_of(id)
    .into_iter()
    .map(|(_, value, point)| (value, metres_along(line, point)))
    .collect()
}

// the node of every reference of a street, in the order of their values
fn reference_nodes_of(id: i64) -> Vec<u64> {
  let mut references = stored_references_of(id);
  references.sort_by_key(|(_, value, _)| *value);
  references.into_iter().map(|(node, _, _)| node).collect()
}

pub(crate) fn stored_number(conn: &rusqlite::Connection, node_id: u64) -> (i64, String) {
  const SQL_SELECT_STORED_NUMBER: &str = "
    SELECT admin_level_id, number
    FROM house_numbers
    WHERE node_id = ?1
  ";

  conn
    .query_row(SQL_SELECT_STORED_NUMBER, [node_id], |row| {
      Ok((row.get(0)?, row.get(1)?))
    })
    .unwrap_or_else(|e| panic!("node {node_id} is not a stored number: {e}"))
}

// what the comparison of a typed number against a stored one ignores: the case of a letter and
// the hyphen of a compound
fn number_key(number: &str) -> String {
  number.to_uppercase().replace('-', "")
}

fn chainage_of(references: &[(u32, f64)], value: u32) -> f64 {
  references
    .iter()
    .find(|(stored, _)| *stored == value)
    .map(|(_, chainage)| *chainage)
    .unwrap_or_else(|| panic!("the street does not carry {value}; {REGENERATE}"))
}

// metres per number between the lowest and the highest stored number, signed along the line
fn factor_of(references: &[(u32, f64)]) -> f64 {
  let lowest = references
    .iter()
    .min_by_key(|(value, _)| *value)
    .expect("a reference");
  let highest = references
    .iter()
    .max_by_key(|(value, _)| *value)
    .expect("a reference");
  (highest.1 - lowest.1) / (f64::from(highest.0) - f64::from(lowest.0))
}

// the way the numbering grows, read from one reference: +1 from the start of the line when the
// reference's distance to the start reads as its number in metres better than to the end
fn numbering_direction(line: &LineString<f64>, chainage: f64, value: u32, meters: f64) -> f64 {
  let as_metres = f64::from(value) * meters;
  let from_start = (chainage - as_metres).abs();
  let from_end = ((line.haversine_length() - chainage) - as_metres).abs();
  if from_start <= from_end { 1.0 } else { -1.0 }
}

// the street with one reference: its line, the metres of the reference along it and the way its
// numbering grows under the given metres per number
fn one_reference_axis(meters: f64) -> (LineString<f64>, f64, f64) {
  let line = street_line(ONE_REFERENCE_STREET_ID);
  let references = references_of(ONE_REFERENCE_STREET_ID, &line);
  let reference = chainage_of(&references, ONE_REFERENCE_NUMBER);
  let direction = numbering_direction(&line, reference, ONE_REFERENCE_NUMBER, meters);
  assert_eq!(
    direction, -1.0,
    "the numbering grows towards the start; {REGENERATE}"
  );
  (line, reference, direction)
}

fn point_at_metres(line: &LineString<f64>, metres: f64) -> Point<f64> {
  let mut remaining = metres.clamp(0.0, line.haversine_length());
  for segment in line.lines() {
    let length = segment.haversine_length();
    if remaining <= length && length > 0.0 {
      return segment
        .line_interpolate_point(remaining / length)
        .expect("a fraction of a segment is a point");
    }
    remaining -= length;
  }
  line.points().next_back().expect("a line has points")
}

fn metres_along(line: &LineString<f64>, point: Point<f64>) -> f64 {
  let mut walked = 0.0;
  let mut nearest: Option<(f64, f64)> = None;
  for segment in line.lines() {
    let fraction = segment.line_locate_point(&point).unwrap_or(0.0);
    let on_segment = segment
      .line_interpolate_point(fraction)
      .expect("a fraction of a segment is a point");
    let distance = point.haversine_distance(&on_segment);
    let length = segment.haversine_length();
    if nearest.is_none_or(|(best, _)| distance < best) {
      nearest = Some((distance, walked + fraction * length));
    }
    walked += length;
  }
  nearest
    .map(|(_, chainage)| chainage)
    .expect("a line has segments")
}

fn answered_point(m: &Value) -> Point<f64> {
  let (latitude, longitude) = point_of(m);
  Point::new(longitude, latitude)
}

fn point_from((latitude, longitude): (f64, f64)) -> Point<f64> {
  Point::new(longitude, latitude)
}

fn lat_lon(point: Point<f64>) -> String {
  format!("{:.6},{:.6}", point.y(), point.x())
}

fn presumed_number(m: &Value) -> f64 {
  number_of(m)
    .and_then(|number| number.parse().ok())
    .unwrap_or_else(|| panic!("no plain number in {m:#}"))
}

// the match of the street, wherever it ranks: by coordinate a crossing street can tie at 0 m
fn street_match<'a>(result: &'a Value, street: &str) -> &'a Value {
  matches(result)
    .iter()
    .find(|m| name_at(m, 12).as_deref() == Some(street))
    .unwrap_or_else(|| panic!("{street} is not among the matches:\n{result:#}"))
}

fn text_answer(preset: &str, query: &str, number: u32, street: &str) -> Value {
  let w = world();
  let result = query_at(w, &w.data_path, preset, &format!("{query} {number}"));
  let top = first(&result);
  assert_eq!(
    name_at(top, 12).as_deref(),
    Some(street),
    "{query} {number} under {preset}: the street ranks first; {REGENERATE}"
  );
  top.clone()
}

fn coordinate_answer(preset: &str, line: &LineString<f64>, metres: f64, street: &str) -> Value {
  let w = world();
  let result = query_at(
    w,
    &w.data_path,
    preset,
    &lat_lon(point_at_metres(line, metres)),
  );
  let m = street_match(&result, street);
  assert_eq!(
    m["coordinates_distance_in_meters"], 0,
    "the point at {metres:.0} m lies on {street}"
  );
  m.clone()
}

pub(crate) fn assert_numbered(m: &Value, number: &str, kind: &str, context: &str) {
  assert_eq!(number_of(m), Some(number), "{context}");
  assert_eq!(kind_of(m), Some(kind), "{context}");
  assert_origin(m, kind, context);
  assert_eq!(
    levels_of(m).last().copied(),
    Some(12),
    "{context}: the ladder ends at the street, the number is the object"
  );
  let label = m["friendly_name"]
    .as_str()
    .expect("friendly_name must be a string");
  assert!(
    label.contains(&format!(", {number}, ")),
    "{context}: the label {label:?} carries no {number}"
  );
}

// the nodes a match names are rows of its own street, and each kind answers the keys its
// arithmetic used: the stored node alone, every reference, the one reference and the metres, or
// the metres alone
fn assert_origin(m: &Value, kind: &str, context: &str) {
  let street = way(street_way(m).expect("a numbered match is a street"));
  let conn = world().open_sqlite();
  let nodes = osm_node_ids(m);
  let stored: Vec<(i64, String)> = nodes
    .iter()
    .map(|&node| stored_number(&conn, node))
    .collect();
  assert!(
    stored.iter().all(|(id, _)| *id == street),
    "{context}: a node off the street in {nodes:?}"
  );
  let metres = meters_per_number(m);
  match kind {
    FROM_OSM_DATA => {
      assert_eq!(stored.len(), 1, "{context}: one stored node");
      assert_eq!(
        number_key(&stored[0].1),
        number_key(number_of(m).unwrap_or_default()),
        "{context}: the node stores the number"
      );
      assert_eq!(metres, None, "{context}: no constant was used");
    }
    MULTIPLE_REFERENCES => {
      assert!(stored.len() >= 2, "{context}: two or more references");
      let mut values: Vec<&str> = stored.iter().map(|(_, number)| number.as_str()).collect();
      values.sort_unstable();
      values.dedup();
      assert_eq!(values.len(), stored.len(), "{context}: one node per value");
      assert_eq!(metres, None, "{context}: no constant was used");
    }
    ONE_REFERENCE => {
      assert_eq!(stored.len(), 1, "{context}: one reference");
      assert!(metres.is_some(), "{context}: the metres per number");
    }
    CONSTANTS => {
      assert!(stored.is_empty(), "{context}: no node");
      assert!(metres.is_some(), "{context}: the metres per number");
    }
    other => panic!("{context}: unknown kind {other}"),
  }
}

fn assert_bare(m: &Value, context: &str) {
  assert!(
    m.get("house_number").is_none(),
    "{context}: no number was asked"
  );
}

fn assert_on_street<G>(point: Point<f64>, street: &G, context: &str)
where
  Point<f64>: EuclideanDistance<f64, G>,
{
  let away = point.euclidean_distance(street);
  assert!(
    away < ON_STREET_TOLERANCE_IN_DEGREES,
    "{context}: the point is {away} degrees off the street"
  );
}

fn assert_placed_at(m: &Value, line: &LineString<f64>, metres: f64, context: &str) {
  let answered = answered_point(m);
  let off = answered.haversine_distance(&point_at_metres(line, metres));
  assert!(
    off <= PLACEMENT_TOLERANCE_IN_METERS,
    "{context}: the point is {off:.1} m from the {metres:.1} m mark"
  );
  assert_on_street(answered, line, context);
}

fn assert_read_back(m: &Value, expected: f64, kind: &str, context: &str) {
  assert_eq!(kind_of(m), Some(kind), "{context}");
  let read = presumed_number(m);
  assert!(
    (read - expected).abs() <= NUMBER_TOLERANCE,
    "{context}: read {read}, expected {expected:.1}"
  );
}

// 00.00. result quality: a stored number answers its own point, from the osm data, on both
// services alike
#[test]
#[ignore]
fn _00_00_a_stored_number_answers_its_stored_point_from_osm_data() {
  let w = world();
  let s = w.start_server();
  let result = w.assert_both(
    &s,
    &ask(&format!("{NUMBERED_STREET_QUERY} 197")),
    &json!({ "matches": [{ "house_number": { "number": "197", "kind": FROM_OSM_DATA } }] }),
  );
  let top = first(&result);
  assert_numbered(top, "197", FROM_OSM_DATA, "197");
  let line = street_line(NUMBERED_STREET_ID);
  let references = references_of(NUMBERED_STREET_ID, &line);
  assert_placed_at(top, &line, chainage_of(&references, 197), "197");
}

// 00.01. result quality: a number between two stored ones lands between their points, as far
// along as its value is between theirs
#[test]
#[ignore]
fn _00_01_a_number_between_two_references_lands_in_proportion_between_them() {
  let line = street_line(NUMBERED_STREET_ID);
  let references = references_of(NUMBERED_STREET_ID, &line);
  for (typed, below, above) in [(210, 197, 232), (133, 70, 197)] {
    let context = format!("{typed} between {below} and {above}");
    let m = text_answer("brazil", NUMBERED_STREET_QUERY, typed, NUMBERED_STREET);
    assert_numbered(&m, &typed.to_string(), MULTIPLE_REFERENCES, &context);
    assert_eq!(
      osm_node_ids(&m),
      reference_nodes_of(NUMBERED_STREET_ID),
      "{context}: every reference of the street, by value"
    );
    let (from, to) = (
      chainage_of(&references, below),
      chainage_of(&references, above),
    );
    let share = f64::from(typed - below) / f64::from(above - below);
    assert_placed_at(&m, &line, from + (to - from) * share, &context);
  }
}

// 00.02. result quality: a number beyond every stored one runs on from the nearest of them, at
// the metres per number the lowest and the highest stored ones set
#[test]
#[ignore]
fn _00_02_a_number_beyond_every_reference_extrapolates_from_the_nearest_extreme() {
  let line = street_line(NUMBERED_STREET_ID);
  let references = references_of(NUMBERED_STREET_ID, &line);
  let factor = factor_of(&references);
  for (typed, anchor) in [(250, 235), (40, 70)] {
    let context = format!("{typed} beyond {anchor}");
    let m = text_answer("brazil", NUMBERED_STREET_QUERY, typed, NUMBERED_STREET);
    assert_numbered(&m, &typed.to_string(), MULTIPLE_REFERENCES, &context);
    let expected =
      chainage_of(&references, anchor) + (f64::from(typed) - f64::from(anchor)) * factor;
    assert_placed_at(&m, &line, expected, &context);
  }
}

// 00.03. result quality: with one stored number the street walks the preset's metres per number
// from it, towards the end its numbering grows to
#[test]
#[ignore]
fn _00_03_a_number_with_one_reference_walks_the_preset_metres_in_the_numbering_direction() {
  let (line, reference, direction) = one_reference_axis(BRAZIL_METERS_PER_NUMBER);
  for typed in [400, 300] {
    let context = format!("{typed} from {ONE_REFERENCE_NUMBER}");
    let m = text_answer(
      "brazil",
      ONE_REFERENCE_STREET_QUERY,
      typed,
      ONE_REFERENCE_STREET,
    );
    assert_numbered(&m, &typed.to_string(), ONE_REFERENCE, &context);
    assert_eq!(
      meters_per_number(&m),
      Some(BRAZIL_METERS_PER_NUMBER),
      "{context}"
    );
    let steps = f64::from(typed) - f64::from(ONE_REFERENCE_NUMBER);
    assert_placed_at(
      &m,
      &line,
      reference + direction * steps * BRAZIL_METERS_PER_NUMBER,
      &context,
    );
  }
}

// 00.04. result quality: with no stored number the street counts the preset's metres per number
// from the start of its line
#[test]
#[ignore]
fn _00_04_a_number_on_a_street_without_references_walks_from_the_start_of_the_line() {
  let line = street_line(BARE_STREET_ID);
  assert!(
    references_of(BARE_STREET_ID, &line).is_empty(),
    "{REGENERATE}"
  );
  for typed in [100, 1] {
    let context = format!("{typed} on a bare street");
    let m = text_answer("brazil", BARE_STREET_QUERY, typed, BARE_STREET);
    assert_numbered(&m, &typed.to_string(), CONSTANTS, &context);
    assert_eq!(
      meters_per_number(&m),
      Some(BRAZIL_METERS_PER_NUMBER),
      "{context}"
    );
    assert_placed_at(
      &m,
      &line,
      f64::from(typed) * BRAZIL_METERS_PER_NUMBER,
      &context,
    );
  }
}

// 00.05. result quality: a number the street is too short for stops at the end its numbering
// grows to, in every presumed scenario
#[test]
#[ignore]
fn _00_05_a_number_past_the_street_is_clamped_at_the_end_the_numbering_grows_towards() {
  for (query, typed, street, id, kind, at_start) in [
    (
      NUMBERED_STREET_QUERY,
      99999,
      NUMBERED_STREET,
      NUMBERED_STREET_ID,
      MULTIPLE_REFERENCES,
      false,
    ),
    (
      ONE_REFERENCE_STREET_QUERY,
      99999,
      ONE_REFERENCE_STREET,
      ONE_REFERENCE_STREET_ID,
      ONE_REFERENCE,
      true,
    ),
    (
      ONE_REFERENCE_STREET_QUERY,
      1,
      ONE_REFERENCE_STREET,
      ONE_REFERENCE_STREET_ID,
      ONE_REFERENCE,
      false,
    ),
    (
      BARE_STREET_QUERY,
      99999,
      BARE_STREET,
      BARE_STREET_ID,
      CONSTANTS,
      false,
    ),
  ] {
    let context = format!("{typed} on {street}");
    let m = text_answer("brazil", query, typed, street);
    assert_numbered(&m, &typed.to_string(), kind, &context);
    let line = street_line(id);
    let end = if at_start {
      0.0
    } else {
      line.haversine_length()
    };
    assert_placed_at(&m, &line, end, &context);
  }
}

// 00.06. result quality: bigger numbers land farther along the street, whatever the scenario
#[test]
#[ignore]
fn _00_06_typed_numbers_land_monotonically_along_the_street() {
  for (query, street, id, typed, grows_from_start) in [
    (
      NUMBERED_STREET_QUERY,
      NUMBERED_STREET,
      NUMBERED_STREET_ID,
      vec![40, 100, 133, 210, 250],
      true,
    ),
    (
      ONE_REFERENCE_STREET_QUERY,
      ONE_REFERENCE_STREET,
      ONE_REFERENCE_STREET_ID,
      vec![100, 300, 400, 500],
      false,
    ),
    (
      BARE_STREET_QUERY,
      BARE_STREET,
      BARE_STREET_ID,
      vec![1, 50, 100, 200],
      true,
    ),
  ] {
    let line = street_line(id);
    let chainages: Vec<f64> = typed
      .iter()
      .map(|&number| {
        metres_along(
          &line,
          answered_point(&text_answer("brazil", query, number, street)),
        )
      })
      .collect();
    let ordered = chainages.windows(2).all(|pair| {
      if grows_from_start {
        pair[0] < pair[1]
      } else {
        pair[0] > pair[1]
      }
    });
    assert!(ordered, "{street}: {typed:?} landed at {chainages:?}");
  }
}

// 00.07. result quality: the metres per number are the preset's — three by default, one under
// brazil — and the scenario is the same under both
#[test]
#[ignore]
fn _00_07_the_metres_per_number_come_from_the_preset() {
  let line = street_line(BARE_STREET_ID);
  for (preset, meters) in [
    ("brazil", BRAZIL_METERS_PER_NUMBER),
    ("default", DEFAULT_METERS_PER_NUMBER),
  ] {
    let context = format!("100 under {preset}");
    let m = text_answer(preset, BARE_STREET_QUERY, 100, BARE_STREET);
    assert_numbered(&m, "100", CONSTANTS, &context);
    assert_eq!(meters_per_number(&m), Some(meters), "{context}");
    assert_placed_at(&m, &line, 100.0 * meters, &context);
  }

  let (line, reference, direction) = one_reference_axis(DEFAULT_METERS_PER_NUMBER);
  let m = text_answer(
    "default",
    ONE_REFERENCE_STREET_QUERY,
    400,
    ONE_REFERENCE_STREET,
  );
  assert_numbered(&m, "400", ONE_REFERENCE, "400 under default");
  assert_eq!(
    meters_per_number(&m),
    Some(DEFAULT_METERS_PER_NUMBER),
    "400 under default"
  );
  let steps = 400.0 - f64::from(ONE_REFERENCE_NUMBER);
  assert_placed_at(
    &m,
    &line,
    reference + direction * steps * DEFAULT_METERS_PER_NUMBER,
    "400 under default",
  );
}

// 00.08. result quality: a presumed number is one answer on both surfaces, from several
// references and from one
#[test]
#[ignore]
fn _00_08_both_surfaces_answer_the_same_presumed_number() {
  let w = world();
  let s = w.start_server();
  for (query, number, kind) in [
    (NUMBERED_STREET_QUERY, "210", MULTIPLE_REFERENCES),
    (ONE_REFERENCE_STREET_QUERY, "400", ONE_REFERENCE),
  ] {
    w.assert_both(
      &s,
      &ask(&format!("{query} {number}")),
      &json!({ "matches": [{ "house_number": { "number": number, "kind": kind } }] }),
    );
  }
}

// 01.00. result quality: a point within 50 m of a stored number reads that number from the osm
// data, and keeps the street's own point and distance
#[test]
#[ignore]
fn _01_00_a_point_within_fifty_metres_of_a_stored_number_reads_it_from_osm_data() {
  let line = street_line(NUMBERED_STREET_ID);
  for (point, distance) in [(NUMBERED_POINT, 0), (NEAR_NUMBERED_POINT, 33)] {
    let asked = point_from(point);
    let context = lat_lon(asked);
    let result = world().run(&[&context]);
    let m = street_match(&result, NUMBERED_STREET);
    assert_numbered(m, "197", FROM_OSM_DATA, &context);
    assert_eq!(
      m["coordinates_distance_in_meters"], distance,
      "{context}; {REGENERATE}"
    );
    assert_placed_at(m, &line, metres_along(&line, asked), &context);
  }
}

// 01.01. result quality: a point between two stored numbers reads a number as far between theirs
// as the point is between their points
#[test]
#[ignore]
fn _01_01_a_point_between_two_references_reads_the_number_in_proportion_of_its_chainage() {
  let line = street_line(NUMBERED_STREET_ID);
  let references = references_of(NUMBERED_STREET_ID, &line);
  let (from, to) = (chainage_of(&references, 70), chainage_of(&references, 197));
  for metres in [130.0, 145.0] {
    let m = coordinate_answer("brazil", &line, metres, NUMBERED_STREET);
    let expected = 70.0 + (197.0 - 70.0) * (metres - from) / (to - from);
    assert_read_back(&m, expected, MULTIPLE_REFERENCES, &format!("{metres} m"));
  }
}

// 01.02. result quality: a point beyond every stored number, and more than 50 m from each, reads
// on from the nearest of them at the metres per number the lowest and the highest stored ones set
#[test]
#[ignore]
fn _01_02_a_point_beyond_every_reference_extrapolates_from_the_nearest_one_in_chainage() {
  for (id, street, metres, anchor) in [
    (NUMBERED_STREET_ID, NUMBERED_STREET, 10.0, 70),
    (MANY_REFERENCES_STREET_ID, MANY_REFERENCES_STREET, 40.0, 208),
  ] {
    let line = street_line(id);
    let references = references_of(id, &line);
    let m = coordinate_answer("brazil", &line, metres, street);
    let expected =
      f64::from(anchor) + (metres - chainage_of(&references, anchor)) / factor_of(&references);
    assert_read_back(
      &m,
      expected,
      MULTIPLE_REFERENCES,
      &format!("{metres} m on {street}"),
    );
  }
}

// 01.03. result quality: a point on a street with one stored number reads it plus the metres
// walked from it, in the direction its numbering grows
#[test]
#[ignore]
fn _01_03_a_point_with_one_reference_counts_metres_from_it_in_the_numbering_direction() {
  let (line, reference, direction) = one_reference_axis(BRAZIL_METERS_PER_NUMBER);
  for metres in [204.0, 404.0] {
    let m = coordinate_answer("brazil", &line, metres, ONE_REFERENCE_STREET);
    let expected =
      f64::from(ONE_REFERENCE_NUMBER) + direction * (metres - reference) / BRAZIL_METERS_PER_NUMBER;
    assert_read_back(&m, expected, ONE_REFERENCE, &format!("{metres} m"));
  }
}

// 01.04. result quality: a point on a street without stored numbers reads the metres from the
// start of its line, at the preset's metres per number
#[test]
#[ignore]
fn _01_04_a_point_on_a_street_without_references_counts_metres_from_the_start_of_the_line() {
  let line = street_line(BARE_STREET_ID);
  for (preset, meters) in [
    ("brazil", BRAZIL_METERS_PER_NUMBER),
    ("default", DEFAULT_METERS_PER_NUMBER),
  ] {
    let m = coordinate_answer(preset, &line, 120.0, BARE_STREET);
    assert_read_back(
      &m,
      120.0 / meters,
      CONSTANTS,
      &format!("120 m under {preset}"),
    );
  }

  let (line, reference, direction) = one_reference_axis(DEFAULT_METERS_PER_NUMBER);
  let m = coordinate_answer("default", &line, 204.0, ONE_REFERENCE_STREET);
  let expected =
    f64::from(ONE_REFERENCE_NUMBER) + direction * (204.0 - reference) / DEFAULT_METERS_PER_NUMBER;
  assert_read_back(&m, expected, ONE_REFERENCE, "204 m under default");
}

// 01.05. result quality: a presumed number is never below one, at the start of a bare line and
// before the lowest stored number alike
#[test]
#[ignore]
fn _01_05_a_presumed_number_is_never_below_one() {
  for (id, street, kind) in [
    (BARE_STREET_ID, BARE_STREET, CONSTANTS),
    (NUMBERED_STREET_ID, NUMBERED_STREET, MULTIPLE_REFERENCES),
  ] {
    let m = coordinate_answer("brazil", &street_line(id), 0.0, street);
    assert_numbered(&m, "1", kind, street);
  }
}

// 01.06. result quality: every street a point answers carries a number, on the street's own
// point and in the order of the distances; two of the ten are named outlines stored as lines,
// and count as streets
#[test]
#[ignore]
fn _01_06_every_street_a_point_answers_carries_a_number_on_its_own_point() {
  let result = world().query_json(&[&lat_lon(point_from(NUMBERED_POINT))]);
  assert_eq!(matches(&result).len(), 10);
  for m in matches(&result) {
    let street = name_at(m, 12).expect("a coordinate match is a street");
    let number = number_of(m).unwrap_or_else(|| panic!("{street} answers no number"));
    assert!(
      !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()),
      "{street}: {number:?} is not a plain number"
    );
    let kind = kind_of(m).unwrap_or_else(|| panic!("{street} answers no kind"));
    assert!(SCENARIOS.contains(&kind), "{street}: {kind:?}");
    assert_numbered(m, number, kind, &street);
    let geometry = Wkt(wkt_at(m, 12).expect("the street carries geometry").as_str())
      .to_geo()
      .expect("the street geometry must be valid wkt");
    assert_on_street(answered_point(m), &geometry, &street);
  }
  let distances = distances(&result);
  assert_eq!(distances[0], 0);
  assert!(
    distances.windows(2).all(|pair| pair[0] <= pair[1]),
    "{distances:?}"
  );
}

// 02.00. result quality: the point a typed number lands on reads the same number back, in the
// same scenario, in every presumed scenario
#[test]
#[ignore]
fn _02_00_a_typed_number_read_back_from_its_point_is_the_same_number() {
  for (query, typed, street, kind) in [
    (
      NUMBERED_STREET_QUERY,
      133,
      NUMBERED_STREET,
      MULTIPLE_REFERENCES,
    ),
    (
      ONE_REFERENCE_STREET_QUERY,
      400,
      ONE_REFERENCE_STREET,
      ONE_REFERENCE,
    ),
    (BARE_STREET_QUERY, 100, BARE_STREET, CONSTANTS),
  ] {
    let context = format!("{street} {typed} read back");
    let placed = text_answer("brazil", query, typed, street);
    assert_eq!(kind_of(&placed), Some(kind), "{context}");
    let w = world();
    let back = query_at(w, &w.data_path, "brazil", &lat_lon(answered_point(&placed)));
    let m = street_match(&back, street);
    // the placed point comes back rounded to five decimals: up to a metre off the street
    assert!(
      m["coordinates_distance_in_meters"]
        .as_u64()
        .is_some_and(|distance| distance <= 1),
      "{context}: {} m from the street",
      m["coordinates_distance_in_meters"]
    );
    assert_read_back(m, f64::from(typed), kind, &context);
  }
}

// 03.00. result quality: a text without a number answers the bare street, on its own point
#[test]
#[ignore]
fn _03_00_a_text_without_a_number_answers_the_bare_street() {
  let result = world().run(&[BARE_STREET_QUERY]);
  let top = first(&result);
  assert_eq!(name_at(top, 12).as_deref(), Some(BARE_STREET));
  assert_bare(top, BARE_STREET_QUERY);
  assert_eq!(
    top["friendly_name"],
    "Rua Bolivar, Boqueirão, Santos, São Paulo, Brasil"
  );
  assert_on_street(
    answered_point(top),
    &street_line(BARE_STREET_ID),
    BARE_STREET_QUERY,
  );
}

// 03.01. result quality: a number is placed on a street only; the area that ranks among the
// streets answers without one, while every street around it carries it
#[test]
#[ignore]
fn _03_01_a_hit_that_is_not_a_street_never_carries_a_number() {
  let query = format!("{NEIGHBOURHOOD_QUERY} {NEIGHBOURHOOD_NUMBER}");
  let result = world().run(&[&query]);
  let area = matches(&result)
    .iter()
    .find(|m| levels_of(m).last().copied() == Some(10))
    .unwrap_or_else(|| panic!("no area among the matches of {query:?}; {REGENERATE}"));
  assert_eq!(
    name_at(area, 10).as_deref(),
    Some(NEIGHBOURHOOD_IN_THE_QUERY)
  );
  assert_bare(area, &query);
  let streets: Vec<&Value> = matches(&result)
    .iter()
    .filter(|m| levels_of(m).contains(&12))
    .collect();
  assert!(!streets.is_empty(), "{REGENERATE}");
  for m in streets {
    let street = name_at(m, 12).expect("a street has a name");
    let kind = kind_of(m).unwrap_or_else(|| panic!("{street} answers no kind"));
    assert!(SCENARIOS.contains(&kind), "{street}: {kind:?}");
    assert_numbered(m, NEIGHBOURHOOD_NUMBER, kind, &street);
  }
}

// 03.02. result quality: a number that is a word of the street's own name is not its house
// number, whether it is the only number typed or not
#[test]
#[ignore]
fn _03_02_a_number_that_is_a_word_of_the_street_name_is_not_a_house_number() {
  let named = world().run(&[NAMED_BY_NUMBER_QUERY]);
  let named: Vec<&Value> = matches(&named)
    .iter()
    .filter(|m| name_at(m, 12).as_deref() == Some(NAMED_BY_NUMBER_STREET))
    .collect();
  assert!(
    !named.is_empty(),
    "{NAMED_BY_NUMBER_STREET} answers; {REGENERATE}"
  );
  for m in named {
    assert_bare(m, NAMED_BY_NUMBER_QUERY);
  }

  let numbered = world().run(&[NAMED_AND_NUMBERED_QUERY]);
  let top = first(&numbered);
  assert_eq!(
    street_way(top),
    Some(NAMED_AND_NUMBERED_WAY),
    "the numbered homonym ranks first; {REGENERATE}"
  );
  assert_numbered(top, "100", ONE_REFERENCE, NAMED_AND_NUMBERED_QUERY);
  for m in matches(&numbered)
    .iter()
    .filter(|m| name_at(m, 12).as_deref() == Some(NAMED_AND_NUMBERED_STREET))
  {
    assert_eq!(number_of(m), Some("100"), "7 is a word of the name");
  }
}

// 03.03. result quality: a street through two neighbourhoods answers once under each, and the
// number typed is the same number on the same point of the street under both
#[test]
#[ignore]
fn _03_03_a_street_on_two_paths_answers_the_number_under_each() {
  let query = format!("{CROSSING_STREET_QUERY} 100");
  let result = world().run(&[&query]);
  let paths: Vec<&Value> = matches(&result)
    .iter()
    .filter(|m| name_at(m, 12).as_deref() == Some(CROSSING_STREET))
    .collect();
  let neighbourhoods: Vec<String> = paths.iter().filter_map(|m| name_at(m, 10)).collect();
  assert_eq!(neighbourhoods, ["Boqueirão", "Embaré"], "{REGENERATE}");
  for (m, neighbourhood) in paths.iter().zip(&neighbourhoods) {
    assert_numbered(m, "100", ONE_REFERENCE, neighbourhood);
  }
  assert_ne!(paths[0]["id"], paths[1]["id"], "two paths are two ids");
  assert_eq!(
    point_of(paths[0]),
    point_of(paths[1]),
    "one street places the number once"
  );
}

// 03.04. result quality: a suffixed number the street does not store is not dropped: it is
// presumed from its digits among the references of the street
#[test]
#[ignore]
fn _03_04_a_suffixed_number_the_street_does_not_store_is_presumed_from_its_digits() {
  let query = format!("{SUFFIXED_STREET_QUERY} 40B");
  let result = world().run(&[&query]);
  let top = first(&result);
  assert_eq!(name_at(top, 12).as_deref(), Some(SUFFIXED_STREET));
  assert_numbered(top, "40B", MULTIPLE_REFERENCES, &query);
}
