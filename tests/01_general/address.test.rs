use crate::common::harness::{encode, get, get_with, record_of};
use crate::common::query::{first, id_of, matches};
use crate::general::world;
use serde_json::{Value, json};

const ANY_TEXT: &str = "rua castro alves";
const NUMBERED_TEXT: &str = "rua castro alves 35";
const ANY_POINT: &str = "-23.970949,-46.318730";

// the levels of a match or of a record as a client reads them, in one order whatever the service
fn ladder_of(m: &Value) -> Vec<Value> {
  let mut levels: Vec<Value> = m["admin_levels"]
    .as_array()
    .expect("admin_levels must be an array")
    .iter()
    .map(|level| {
      json!({
        "level": level["level"],
        "name": level["name"],
        "post_code": level["post_code"],
        "osm_relation_id": level["osm_relation_id"],
        "osm_way_id": level["osm_way_id"],
        "osm_merged_way_ids": level["osm_merged_way_ids"],
      })
    })
    .collect();
  levels.sort_by_key(Value::to_string);
  levels
}

fn text_answer(port: u16) -> Value {
  get(port, &format!("/geocode?query={}", encode(ANY_TEXT))).json()
}

fn numbered_answer(port: u16) -> Value {
  get(port, &format!("/geocode?query={}", encode(NUMBERED_TEXT))).json()
}

fn point_answer(port: u16) -> Value {
  get(port, &format!("/geocode?query={}", encode(ANY_POINT))).json()
}

fn path_of(id: &str) -> String {
  format!("/address?id={}", encode(id))
}

// 00.00. contract: the record of a text match is the match again, every level with its shape and
// nothing of the question
#[test]
#[ignore]
fn _00_00_the_record_of_a_text_match_carries_the_shape_of_every_level() {
  let s = world().start_server();
  let answer = text_answer(s.port);
  let top = first(&answer);
  let record = record_of(s.port, top);
  assert_eq!(record["id"], top["id"]);
  assert_eq!(ladder_of(&record), ladder_of(top));
  assert_eq!(record["attributes"], top["attributes"]);
  for level in record["admin_levels"]
    .as_array()
    .expect("admin_levels must be an array")
  {
    assert!(
      level["wkt"].is_string(),
      "every level of a record has its shape: {level}"
    );
  }
  for level in top["admin_levels"]
    .as_array()
    .expect("admin_levels must be an array")
  {
    assert!(
      level.get("wkt").is_none(),
      "no level of a match has one: {level}"
    );
  }
  assert!(record.get("house_number").is_none());
  assert!(record["similarity"].is_null());
  assert!(record["score"].is_null());
  assert!(record["coordinates_distance_in_meters"].is_null());
}

// 00.01. contract: the record of a coordinate match is the same ladder under the same id, with
// the number the point answered placed on the street again: on the number's own point, where the
// match answered the point of the street closest to the question
#[test]
#[ignore]
fn _00_01_the_record_of_a_coordinate_match_places_its_number_again() {
  let s = world().start_server();
  let answer = point_answer(s.port);
  let top = first(&answer);
  assert!(
    top["house_number"].is_object(),
    "a point answers a number: {top}"
  );
  let record = record_of(s.port, top);
  assert_eq!(record["id"], top["id"]);
  assert_eq!(ladder_of(&record), ladder_of(top));
  assert_eq!(record["house_number"], top["house_number"]);
  assert_eq!(record["friendly_name"], top["friendly_name"]);
  assert!(record["coordinates_distance_in_meters"].is_null());
  for axis in ["latitude", "longitude"] {
    let apart = (record[axis].as_f64().unwrap() - top[axis].as_f64().unwrap()).abs();
    assert!(
      apart < 0.001,
      "{axis}: the number's own point sits on the same street, {apart} degrees from the closest point the match answered"
    );
  }
  for level in record["admin_levels"]
    .as_array()
    .expect("admin_levels must be an array")
  {
    assert!(level["wkt"].is_string(), "{level}");
  }
}

// 00.06. contract: a numbered text match names its number in the id, and its record places it on
// the same point under the same label
#[test]
#[ignore]
fn _00_06_the_record_of_a_numbered_text_match_carries_the_number() {
  let s = world().start_server();
  let answer = numbered_answer(s.port);
  let top = first(&answer);
  assert_eq!(top["house_number"]["number"], "35", "{top}");
  let id = id_of(top);
  assert!(id.ends_with(".n35"), "the number comes last: {id}");
  let bare = &id[..id.len() - ".n35".len()];
  let record = record_of(s.port, top);
  assert_eq!(record["id"], top["id"]);
  assert_eq!(record["house_number"], top["house_number"]);
  assert_eq!(record["latitude"], top["latitude"]);
  assert_eq!(record["longitude"], top["longitude"]);
  assert_eq!(record["friendly_name"], top["friendly_name"]);

  let street = get(s.port, &path_of(bare)).json();
  assert_eq!(street["id"], bare, "the same street without its number");
  assert!(street.get("house_number").is_none());
  assert_ne!(street["friendly_name"], top["friendly_name"]);
}

// 00.02. contract: every id an answer gives is found again
#[test]
#[ignore]
fn _00_02_every_id_of_an_answer_is_found_again() {
  let s = world().start_server();
  let answer = point_answer(s.port);
  assert_eq!(matches(&answer).len(), 10);
  for m in matches(&answer) {
    assert_eq!(record_of(s.port, m)["id"], m["id"]);
  }
}

// 00.03. contract: an id spells a path the hierarchy must know; the elements out of order, the
// root dropped or a number on a country is no address, and case does not matter
#[test]
#[ignore]
fn _00_03_a_forged_path_is_not_found_and_case_does_not_matter() {
  let s = world().start_server();
  let answer = text_answer(s.port);
  let id = id_of(first(&answer));
  let segments: Vec<&str> = id.split('.').collect();
  assert!(segments.len() >= 3, "a street under its country: {id}");
  let (iso, elements) = (segments[0], &segments[1..]);

  let mut reversed: Vec<&str> = elements.to_vec();
  reversed.reverse();
  let forged = [
    format!("{iso}.{}", reversed.join(".")),
    format!("{iso}.{}", elements[1..].join(".")),
    format!("{iso}.{}.{}", elements[0], elements.join(".")),
    format!("{iso}.{}.n10", elements[0]),
  ];
  for forged in &forged {
    let r = get(s.port, &path_of(forged));
    assert_eq!(r.status, 404, "{forged}: {}", r.text());
    assert_eq!(r.text(), r#"{"error":"address not found"}"#);
  }

  let upper = get(s.port, &path_of(&id.to_ascii_uppercase()));
  assert_eq!(upper.status, 200, "{}", upper.text());
  assert_eq!(
    upper.json()["id"],
    id,
    "the record spells its id in lowercase"
  );
}

// 00.04. etag: a record carries the etag of its id and the version, answers 304 to it before any
// shape is read, and the same whatever the case of the id
#[test]
#[ignore]
fn _00_04_a_record_carries_an_etag_of_its_id_and_answers_not_modified() {
  let s = world().start_server();
  let answer = point_answer(s.port);
  let ids: Vec<&str> = matches(&answer).iter().map(id_of).collect();
  let r = get(s.port, &path_of(ids[0]));
  assert_eq!(r.status, 200);
  let etag = r
    .header("etag")
    .expect("a record carries an etag")
    .to_string();

  let again = get_with(s.port, &path_of(ids[0]), &[("If-None-Match", &etag)]);
  assert_eq!(again.status, 304);
  assert!(again.body.is_empty(), "a 304 carries no body");
  assert_eq!(again.header("etag"), Some(etag.as_str()));

  let upper = get(s.port, &path_of(&ids[0].to_ascii_uppercase()));
  assert_eq!(
    upper.header("etag"),
    Some(etag.as_str()),
    "one etag whatever the case"
  );

  let other = get(s.port, &path_of(ids[1]));
  assert_eq!(other.status, 200);
  assert_ne!(
    other.header("etag"),
    Some(etag.as_str()),
    "another id, another etag"
  );
}
