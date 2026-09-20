use geo::Geometry;
use rusqlite::Connection;

use crate::common::harness::{decode_wkb, merged_way_ids_of};
use crate::common::query::{first, level_at, matches, name_at};
use crate::santos::world;

const BRASIL: i64 = 118_941;
const SAO_PAULO: i64 = 596_409;
const SANTOS: i64 = 596_885;
const APARECIDA: i64 = 8_148_001;
const EMBARE: i64 = 8_565_765;
const BOQUEIRAO: i64 = 8_565_761;
const JOSE_MENINO: i64 = 8_565_771;
const MARAPE: i64 = 8_565_773;
const STREETS: i64 = 7_195;
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

fn parents_of(conn: &Connection, id: i64) -> Vec<i64> {
  conn
    .prepare(
      "SELECT parent_id FROM admin_levels_hierarchy \
       WHERE admin_level_id = ?1 AND parent_id IS NOT NULL ORDER BY parent_id",
    )
    .expect("failed to prepare")
    .query_map([id], |r| r.get(0))
    .expect("failed to query")
    .map(|r| r.expect("failed to read a parent id"))
    .collect()
}

// the paths the domain enumerates, climbed here from the edges: most specific first, one per
// parent, and an empty path for a root
fn paths_of(conn: &Connection, id: i64) -> Vec<Vec<i64>> {
  let parents = parents_of(conn, id);
  if parents.is_empty() {
    assert!(
      has_row(conn, id),
      "no hierarchy row for {id}: a resolved area always has one"
    );
    return vec![vec![]];
  }
  let mut paths: Vec<Vec<i64>> = Vec::new();
  for parent in parents {
    for tail in paths_of(conn, parent) {
      let mut path = vec![parent];
      path.extend(tail);
      paths.push(path);
    }
  }
  paths
}

fn has_row(conn: &Connection, id: i64) -> bool {
  conn
    .query_row(
      "SELECT COUNT(*) FROM admin_levels_hierarchy WHERE admin_level_id = ?1",
      [id],
      |r| r.get::<_, i64>(0),
    )
    .expect("failed to count the rows of an area")
    > 0
}

fn names_of(conn: &Connection, path: &[i64]) -> Vec<String> {
  path
    .iter()
    .map(|id| {
      conn
        .query_row("SELECT name FROM admin_levels WHERE id = ?1", [id], |r| {
          r.get(0)
        })
        .unwrap_or_else(|e| panic!("no admin_levels row {id}: {e}"))
    })
    .collect()
}

// the label is composed at query time now, so the query is where it is read
fn friendly_name(query: &str) -> String {
  first(&world().run(&[query]))["friendly_name"]
    .as_str()
    .expect("a match must carry a friendly name")
    .to_string()
}

fn geometry_of(conn: &Connection, id: i64) -> Geometry<f64> {
  let wkb: Vec<u8> = conn
    .query_row("SELECT wkb FROM admin_levels WHERE id = ?1", [id], |r| {
      r.get(0)
    })
    .unwrap_or_else(|e| panic!("no admin_levels row {id}: {e}"));
  decode_wkb(&wkb)
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

// 00.00. the rows: a street is a line, or a fold of lines, even when the way closes on itself
#[test]
#[ignore]
fn _00_00_every_street_is_a_line_or_a_fold_of_lines_even_when_the_way_is_a_ring() {
  let conn = world().open_sqlite();
  let streets = ids_where(&conn, "SELECT id FROM admin_levels WHERE admin_level = 12");
  assert_eq!(streets.len() as i64, STREETS, "{REGENERATE}");
  for id in &streets {
    match (geometry_of(&conn, *id), merged_way_ids_of(&conn, *id)) {
      (Geometry::LineString(_), None) => {}
      (Geometry::MultiLineString(lines), Some(ways)) => {
        assert_eq!(lines.0.len(), ways.len(), "street {id}: one way per line");
        assert_eq!(way(ways[0]), *id, "street {id}: its own way comes first");
      }
      (other, ways) => {
        panic!("street {id} is neither a line nor a traced fold: {other:?}, {ways:?}")
      }
    }
  }
  match geometry_of(&conn, way(92_741_038)) {
    Geometry::MultiLineString(lines) => {
      let ring = lines
        .0
        .iter()
        .find(|line| line.0.len() == 31)
        .unwrap_or_else(|| panic!("the ring of Praça da Paz is one of the lines; {REGENERATE}"));
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
    friendly_name("11380-500"),
    "Avenida Monteiro Lobato, São Paulo, Brasil, 11380-500"
  );
  let with_post_code = ids_where(
    &conn,
    "SELECT id FROM admin_levels WHERE admin_level = 12 AND post_code IS NOT NULL",
  );
  assert!(with_post_code.len() > 100, "{REGENERATE}");
}

// 00.03. the rows: every row comes from exactly one way or one relation, and its id packs that
// origin
#[test]
#[ignore]
fn _00_03_every_row_carries_one_origin_packed_in_its_id() {
  let conn = world().open_sqlite();
  for (sql, broken) in [
    (
      "SELECT COUNT(*) FROM admin_levels WHERE (relation_id IS NULL) = (way_id IS NULL)",
      "rows with no origin or with two",
    ),
    (
      "SELECT COUNT(*) FROM admin_levels \
       WHERE id != COALESCE(relation_id, way_id) * 2 + (relation_id IS NOT NULL)",
      "rows whose id does not pack their origin",
    ),
  ] {
    let count: i64 = conn
      .query_row(sql, [], |r| r.get(0))
      .expect("failed to count");
    assert_eq!(count, 0, "{broken}");
  }
}

// 01.00. chains: the country is the root, and every state hangs from it alone
#[test]
#[ignore]
fn _01_00_the_country_is_the_root_and_every_state_hangs_from_it() {
  let conn = world().open_sqlite();
  assert_eq!(paths_of(&conn, BRASIL), vec![Vec::<i64>::new()]);

  let states: Vec<i64> = ids_where(
    &conn,
    "SELECT id FROM admin_levels WHERE admin_level = 4 ORDER BY name",
  );
  assert_eq!(states.len(), STATES, "{REGENERATE}");
  for id in states {
    assert_eq!(paths_of(&conn, id), vec![vec![BRASIL]]);
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
    paths_of(&conn, way(169_924_327)),
    vec![vec![SAO_PAULO, BRASIL]],
    "the street in Cubatão attaches to the state, skipping its city"
  );
}

// 01.02. chains: five segments with one name, three labels
#[test]
#[ignore]
fn _01_02_homonyms_get_ancestries_that_tell_them_apart() {
  let conn = world().open_sqlite();
  let segments = ids_where(
    &conn,
    "SELECT id FROM admin_levels WHERE name = 'Rua Castro Alves' ORDER BY id",
  );
  let mut ancestries: Vec<String> = segments
    .iter()
    .flat_map(|&id| paths_of(&conn, id))
    .map(|path| names_of(&conn, &path).join(", "))
    .collect();
  ancestries.sort_unstable();
  ancestries.dedup();

  assert_eq!(
    ancestries,
    vec![
      "Embaré, Santos, São Paulo, Brasil",
      "Guarujá, São Paulo, Brasil",
      "São Paulo, Brasil",
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
    paths_of(&conn, way(196_616_079)),
    vec![vec![APARECIDA, SANTOS, SAO_PAULO, BRASIL]],
    "{REGENERATE}"
  );
  let paths = paths_of(&conn, way(185_852_085));
  assert_eq!(paths.len(), 1);
  assert_eq!(paths[0].len(), 5);
  assert_eq!(
    paths[0][0],
    way(196_616_079),
    "the innermost neighbourhood comes first"
  );
  assert_eq!(
    friendly_name("rua aureliano coutinho conjunto habitacional jau"),
    "Rua Aureliano Coutinho, Conjunto Habitacional Jaú, Aparecida, Santos, São Paulo, Brasil"
  );
}

// 01.04. chains: the chain is sparse — no neighbourhood around, straight to the city
#[test]
#[ignore]
fn _01_04_a_street_outside_every_neighbourhood_attaches_to_its_city() {
  let conn = world().open_sqlite();
  assert_eq!(
    paths_of(&conn, way(360_562_735)),
    vec![vec![SANTOS, SAO_PAULO, BRASIL]],
    "{REGENERATE}"
  );
}

// 01.05. chains: a neighbourhood from a relation and one from a place way resolve alike
#[test]
#[ignore]
fn _01_05_neighbourhoods_from_relations_and_from_ways_resolve_alike() {
  let conn = world().open_sqlite();
  let under_santos = vec![vec![SANTOS, SAO_PAULO, BRASIL]];
  assert_eq!(
    paths_of(&conn, relation(4_074_000)),
    under_santos,
    "{REGENERATE}"
  );
  assert_eq!(paths_of(&conn, way(1_223_042_714)), under_santos);
}

// 02.00. search: the ancestry is indexed with the name, and tells homonym streets apart; a street
// that crosses two neighbourhoods answers once under each of them
#[test]
#[ignore]
fn _02_00_the_ancestry_tells_homonym_streets_apart() {
  for (query, neighbourhood, way) in [
    ("rua bento de abreu boqueirao", "Boqueirão", 255_734_641),
    ("rua bento de abreu embare", "Embaré", 255_734_641),
  ] {
    let result = world().run(&[query]);
    let top = first(&result);
    assert_eq!(name_at(top, 10).as_deref(), Some(neighbourhood), "{query}");
    assert_eq!(street_way_of(top), way, "{query}; {REGENERATE}");
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
    assert_eq!(street_way_of(top), 48_458_023, "{query}; {REGENERATE}");
  }
}

// 03.00. the tree: a street traced over the line two neighbourhoods share hangs from both
#[test]
#[ignore]
fn _03_00_a_street_along_a_shared_boundary_hangs_from_both_neighbourhoods() {
  let conn = world().open_sqlite();

  assert_eq!(
    paths_of(&conn, way(316_743_190)),
    vec![
      vec![JOSE_MENINO, SANTOS, SAO_PAULO, BRASIL],
      vec![MARAPE, SANTOS, SAO_PAULO, BRASIL]
    ],
    "{REGENERATE}"
  );
}

// 03.01. the tree: a street crossing two neighbourhoods answers once per path
#[test]
#[ignore]
fn _03_01_a_street_crossing_two_neighbourhoods_answers_one_match_per_path() {
  let conn = world().open_sqlite();
  assert_eq!(
    paths_of(&conn, way(255_734_641)),
    vec![
      vec![BOQUEIRAO, SANTOS, SAO_PAULO, BRASIL],
      vec![EMBARE, SANTOS, SAO_PAULO, BRASIL]
    ],
    "{REGENERATE}"
  );

  let answers = world().run(&["rua bento de abreu"]);
  let crossing: Vec<&serde_json::Value> = matches(&answers)
    .iter()
    .filter(|m| street_way_of(m) == 255_734_641)
    .collect();
  assert_eq!(crossing.len(), 2, "one match per path");
  let neighbourhoods: Vec<Option<String>> = crossing.iter().map(|m| name_at(m, 10)).collect();
  assert_eq!(
    neighbourhoods,
    vec![Some("Boqueirão".to_string()), Some("Embaré".to_string())]
  );
  assert_ne!(crossing[0]["id"], crossing[1]["id"], "one id per path");
}

// 03.02. the tree: the country is the only root, and an area lists the areas directly inside it
#[test]
#[ignore]
fn _03_02_the_country_is_the_only_root_and_an_area_lists_what_is_directly_inside_it() {
  let conn = world().open_sqlite();
  assert_eq!(
    ids_where(
      &conn,
      "SELECT admin_level_id FROM admin_levels_hierarchy WHERE parent_id IS NULL",
    ),
    vec![BRASIL]
  );

  let neighbourhoods_of_santos = ids_where(
    &conn,
    "SELECT al.id FROM admin_levels al \
     WHERE al.admin_level = 10 \
       AND al.id IN (SELECT admin_level_id FROM admin_levels_hierarchy WHERE parent_id = 596885) \
     ORDER BY al.id",
  );
  for id in [EMBARE, BOQUEIRAO, APARECIDA] {
    assert!(
      neighbourhoods_of_santos.contains(&id),
      "{id} is in Santos; {REGENERATE}"
    );
  }
  assert!(
    !neighbourhoods_of_santos.contains(&way(196_616_079)),
    "the neighbourhood inside Aparecida hangs from it, not from the city"
  );
  assert_eq!(
    parents_of(&conn, way(196_616_079)),
    vec![APARECIDA],
    "one step down from Aparecida"
  );
}
