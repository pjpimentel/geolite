use geo::Geometry;
use geozero::{ToGeo, wkb::SpatiaLiteWkb};
use rusqlite::Connection;

use crate::common::query::{first, level_at, name_at};
use crate::santos::world;

const BRASIL: i64 = 118_941;
const SAO_PAULO: i64 = 596_409;
const SANTOS: i64 = 596_885;
const APARECIDA: i64 = 8_148_001;
const STREETS: i64 = 12_878;
const STATES: usize = 27;
const PLACE_WAYS: usize = 22;

const REGENERATE: &str = "the fixture changed; regenerate deliberately and update the constants";

// the packed ids of `admin_level::id`: a way is its osm id shifted left, a relation has the low bit
fn way(osm_id: u64) -> i64 {
  (osm_id << 1) as i64
}

fn relation(osm_id: u64) -> i64 {
  ((osm_id << 1) | 1) as i64
}

fn chain_of(conn: &Connection, id: i64) -> (Vec<i64>, String) {
  let (chain, label): (String, String) = conn
    .query_row(
      "SELECT json(ancestor_ids), user_friendly_name FROM admin_levels_hierarchy \
       WHERE admin_level_id = ?1",
      [id],
      |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap_or_else(|e| panic!("no hierarchy row for {id}: {e}"));
  (
    serde_json::from_str(&chain).expect("ancestor_ids must be a json array"),
    label,
  )
}

fn geometry_of(conn: &Connection, id: i64) -> Geometry<f64> {
  let wkb: Vec<u8> = conn
    .query_row("SELECT wkb FROM admin_levels WHERE id = ?1", [id], |r| {
      r.get(0)
    })
    .unwrap_or_else(|e| panic!("no admin_levels row {id}: {e}"));
  SpatiaLiteWkb(wkb.as_slice())
    .to_geo()
    .unwrap_or_else(|e| panic!("row {id} has an undecodable geometry: {e}"))
}

fn ids_where(conn: &Connection, sql: &str) -> Vec<i64> {
  conn
    .prepare(sql)
    .expect("failed to prepare")
    .query_map([], |r| r.get(0))
    .expect("failed to query")
    .map(|r| r.expect("failed to read an id"))
    .collect()
}

fn street_way_of(m: &serde_json::Value) -> u64 {
  level_at(m, 12)
    .and_then(|a| a["osm_way_id"].as_u64())
    .unwrap_or_else(|| panic!("the top match must be a street: {m}"))
}

// 00.00. the rows: a street is always a line, even when the way closes on itself
#[test]
#[ignore]
fn _00_00_every_street_is_a_line_even_when_the_way_is_a_ring() {
  let conn = world().open_sqlite();
  let streets = ids_where(&conn, "SELECT id FROM admin_levels WHERE admin_level = 12");
  assert_eq!(streets.len() as i64, STREETS, "{REGENERATE}");
  for id in &streets {
    assert!(
      matches!(geometry_of(&conn, *id), Geometry::LineString(_)),
      "street {id} is not a line"
    );
  }
  match geometry_of(&conn, way(92_741_038)) {
    Geometry::LineString(ring) => {
      assert_eq!(ring.0.len(), 31, "{REGENERATE}");
      assert_eq!(
        ring.0.first(),
        ring.0.last(),
        "Praça da Paz closes on itself"
      );
    }
    other => panic!("Praça da Paz must stay a line, got {other:?}"),
  }
}

// 00.01. the rows: a closed place way becomes a polygon, a clipped boundary stays lines
#[test]
#[ignore]
fn _00_01_a_place_way_closes_into_a_polygon_and_a_clipped_boundary_stays_a_line() {
  let conn = world().open_sqlite();
  let place_ways = ids_where(
    &conn,
    "SELECT id FROM admin_levels WHERE admin_level = 10 AND way_id IS NOT NULL",
  );
  assert_eq!(place_ways.len(), PLACE_WAYS, "{REGENERATE}");
  assert!(
    place_ways.contains(&way(1_223_042_714)),
    "Alemoa is a place way"
  );
  for id in &place_ways {
    assert!(
      matches!(geometry_of(&conn, *id), Geometry::MultiPolygon(_)),
      "place way {id} did not close into a polygon"
    );
  }
  assert!(
    matches!(
      geometry_of(&conn, relation(5_216_124)),
      Geometry::MultiLineString(_)
    ),
    "Antártica is clipped by the extract and never closes"
  );
}

// 00.02. the rows: the post code of the way is kept, and the label ends with it
#[test]
#[ignore]
fn _00_02_a_street_keeps_the_post_code_of_its_way_and_its_label_ends_with_it() {
  let conn = world().open_sqlite();
  let post_code: Option<String> = conn
    .query_row(
      "SELECT post_code FROM admin_levels WHERE id = ?1",
      [way(48_458_023)],
      |r| r.get(0),
    )
    .expect("the avenue must be extracted");
  assert_eq!(post_code.as_deref(), Some("11380-500"), "{REGENERATE}");
  assert_eq!(
    chain_of(&conn, way(48_458_023)).1,
    "Avenida Monteiro Lobato, São Paulo, Brasil, 11380-500"
  );
  let with_post_code = ids_where(
    &conn,
    "SELECT id FROM admin_levels WHERE admin_level = 12 AND post_code IS NOT NULL",
  );
  assert!(with_post_code.len() > 100, "{REGENERATE}");
}

// 01.00. chains: the country is the root, and every state hangs from it alone
#[test]
#[ignore]
fn _01_00_the_country_is_the_root_and_every_state_hangs_from_it() {
  let conn = world().open_sqlite();
  assert_eq!(chain_of(&conn, BRASIL), (vec![], "Brasil".to_string()));

  let states: Vec<(i64, String)> = conn
    .prepare("SELECT id, name FROM admin_levels WHERE admin_level = 4 ORDER BY name")
    .expect("failed to prepare")
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
    .expect("failed to query")
    .map(|r| r.expect("failed to read a state"))
    .collect();
  assert_eq!(states.len(), STATES, "{REGENERATE}");
  for (id, name) in states {
    assert_eq!(
      chain_of(&conn, id),
      (vec![BRASIL], format!("{name}, Brasil"))
    );
  }
}

// 01.01. chains: a boundary that never closed cannot contain anything
#[test]
#[ignore]
fn _01_01_a_clipped_city_never_becomes_an_ancestor() {
  let conn = world().open_sqlite();
  assert!(
    matches!(
      geometry_of(&conn, relation(298_437)),
      Geometry::MultiLineString(_)
    ),
    "Cubatão is clipped by the extract; {REGENERATE}"
  );
  assert_eq!(
    chain_of(&conn, way(169_924_327)),
    (
      vec![SAO_PAULO, BRASIL],
      "Rua Castro Alves, São Paulo, Brasil".to_string()
    ),
    "the street in Cubatão attaches to the state, skipping its city"
  );
}

// 01.02. chains: five segments with one name, three labels
#[test]
#[ignore]
fn _01_02_homonyms_get_labels_that_tell_them_apart() {
  let conn = world().open_sqlite();
  let labels: Vec<String> = conn
    .prepare(
      "SELECT DISTINCT h.user_friendly_name FROM admin_levels al \
       JOIN admin_levels_hierarchy h ON h.admin_level_id = al.id \
       WHERE al.name = 'Rua Castro Alves' ORDER BY 1",
    )
    .expect("failed to prepare")
    .query_map([], |r| r.get(0))
    .expect("failed to query")
    .map(|r| r.expect("failed to read a label"))
    .collect();
  assert_eq!(
    labels,
    vec![
      "Rua Castro Alves, Embaré, Santos, São Paulo, Brasil",
      "Rua Castro Alves, Guarujá, São Paulo, Brasil",
      "Rua Castro Alves, São Paulo, Brasil",
    ],
    "{REGENERATE}"
  );
}

// 01.03. chains: an area inside an area of the same level chains through it
#[test]
#[ignore]
fn _01_03_a_neighbourhood_inside_a_neighbourhood_chains_through_it() {
  let conn = world().open_sqlite();
  assert_eq!(
    chain_of(&conn, way(196_616_079)),
    (
      vec![APARECIDA, SANTOS, SAO_PAULO, BRASIL],
      "Conjunto Habitacional Jaú, Aparecida, Santos, São Paulo, Brasil".to_string()
    ),
    "{REGENERATE}"
  );
  let (chain, label) = chain_of(&conn, way(185_852_085));
  assert_eq!(chain.len(), 5);
  assert_eq!(
    chain[0],
    way(196_616_079),
    "the innermost neighbourhood comes first"
  );
  assert_eq!(
    label,
    "Rua Aureliano Coutinho, Conjunto Habitacional Jaú, Aparecida, Santos, São Paulo, Brasil"
  );
}

// 01.04. chains: the chain is sparse — no neighbourhood around, straight to the city
#[test]
#[ignore]
fn _01_04_a_street_outside_every_neighbourhood_attaches_to_its_city() {
  let conn = world().open_sqlite();
  assert_eq!(
    chain_of(&conn, way(360_562_735)),
    (
      vec![SANTOS, SAO_PAULO, BRASIL],
      "Avenida Brasil, Santos, São Paulo, Brasil".to_string()
    ),
    "{REGENERATE}"
  );
}

// 01.05. chains: a neighbourhood from a relation and one from a place way resolve alike
#[test]
#[ignore]
fn _01_05_neighbourhoods_from_relations_and_from_ways_resolve_alike() {
  let conn = world().open_sqlite();
  let under_santos = vec![SANTOS, SAO_PAULO, BRASIL];
  assert_eq!(
    chain_of(&conn, relation(4_074_000)),
    (
      under_santos.clone(),
      "Aparecida, Santos, São Paulo, Brasil".to_string()
    ),
    "{REGENERATE}"
  );
  assert_eq!(
    chain_of(&conn, way(1_223_042_714)),
    (
      under_santos,
      "Alemoa, Santos, São Paulo, Brasil".to_string()
    )
  );
}

// 02.00. search: the ancestry is indexed with the name, and tells homonym streets apart
#[test]
#[ignore]
fn _02_00_the_ancestry_tells_homonym_streets_apart() {
  for (query, neighbourhood, ways) in [
    (
      "rua bento de abreu boqueirao",
      "Boqueirão",
      [255_734_641, 883_674_520],
    ),
    (
      "rua bento de abreu embare",
      "Embaré",
      [485_448_287, 883_674_521],
    ),
  ] {
    let result = world().run(&[query]);
    let top = first(&result);
    assert_eq!(name_at(top, 10).as_deref(), Some(neighbourhood), "{query}");
    let way_id = street_way_of(top);
    assert!(
      ways.contains(&way_id),
      "{query} landed on way {way_id}; {REGENERATE}"
    );
  }
}

// 02.01. search: the post code is indexed as written and digits-only
#[test]
#[ignore]
fn _02_01_a_post_code_finds_its_street_in_both_written_forms() {
  for query in ["11380-500", "11380500"] {
    let result = world().run(&[query]);
    let top = first(&result);
    assert_eq!(
      name_at(top, 12).as_deref(),
      Some("Avenida Monteiro Lobato"),
      "{query}: {top}"
    );
    let way_id = street_way_of(top);
    assert!(
      [48_458_023, 484_439_194].contains(&way_id),
      "{query} landed on way {way_id}; {REGENERATE}"
    );
  }
}
