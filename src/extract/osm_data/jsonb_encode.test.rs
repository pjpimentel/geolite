use super::*;

use crate::extract::osm_data::osm_nodes::osm_node;
use crate::extract::osm_data::osm_relations::{
  osm_member_type, osm_relation, osm_relation_member,
};
use crate::extract::osm_data::osm_ways::osm_way;

// o proprio sqlite e o oraculo do formato: se o jsonb estiver malformado,
// JSON() falha ou devolve algo diferente do esperado
fn to_json(payload: &[u8]) -> serde_json::Value {
  let conn = rusqlite::Connection::open_in_memory().expect("failed to open sqlite");
  let text: String = conn
    .query_row("SELECT JSON(?1)", rusqlite::params![payload], |r| r.get(0))
    .expect("sqlite nao conseguiu ler o jsonb produzido");
  serde_json::from_str(&text).expect("sqlite devolveu json invalido")
}

fn tags(pairs: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
  pairs
    .iter()
    .map(|&(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

fn encode_text_object(value: &str) -> serde_json::Value {
  let mut enc = encoder::new();
  let mut out = Vec::new();
  enc.encode_osm_node(
    &mut out,
    &osm_node {
      id: 1,
      lat: 0.0,
      lon: 0.0,
      tags: tags(&[("k", value)]),
    },
  );
  to_json(&out)
}

// 00.00: node vira objeto com lat, lon e tags
#[test]
fn _00_00_encodes_node_with_lat_lon_and_tags() {
  let mut enc = encoder::new();
  let mut out = Vec::new();
  enc.encode_osm_node(
    &mut out,
    &osm_node {
      id: 7,
      lat: 38.7,
      lon: -9.1,
      tags: tags(&[("name", "Marco Zero")]),
    },
  );

  let json = to_json(&out);
  assert!((json["lat"].as_f64().expect("lat") - 38.7).abs() < 1e-9);
  assert!((json["lon"].as_f64().expect("lon") - -9.1).abs() < 1e-9);
  assert_eq!(json["tags"]["name"], "Marco Zero");
}

// 00.01: node sem tags produz um objeto tags vazio, nao ausente
#[test]
fn _00_01_encodes_node_without_tags_as_empty_object() {
  let mut enc = encoder::new();
  let mut out = Vec::new();
  enc.encode_osm_node(
    &mut out,
    &osm_node {
      id: 8,
      lat: 0.0,
      lon: 0.0,
      tags: std::collections::HashMap::new(),
    },
  );

  assert_eq!(to_json(&out)["tags"], serde_json::json!({}));
}

// 00.02: way vira objeto com array de refs e objeto de tags
#[test]
fn _00_02_encodes_way_with_refs_array() {
  let mut enc = encoder::new();
  let mut out = Vec::new();
  enc.encode_osm_way(
    &mut out,
    &osm_way {
      id: 100,
      refs: vec![1, 2, -3],
      tags: tags(&[("highway", "residential")]),
    },
  );

  let json = to_json(&out);
  assert_eq!(json["refs"], serde_json::json!([1, 2, -3]));
  assert_eq!(json["tags"]["highway"], "residential");
}

// 00.03: relation carrega os tres tipos de membro, cada um com sua sigla
#[test]
fn _00_03_encodes_relation_with_every_member_type() {
  let mut enc = encoder::new();
  let mut out = Vec::new();
  enc.encode_osm_relation(
    &mut out,
    &osm_relation {
      id: 200,
      tags: tags(&[("name", "Lisboa")]),
      members: vec![
        osm_relation_member {
          osm_member_type: osm_member_type::node,
          id: 1,
          role: "admin_centre".to_string(),
        },
        osm_relation_member {
          osm_member_type: osm_member_type::way,
          id: 2,
          role: "outer".to_string(),
        },
        osm_relation_member {
          osm_member_type: osm_member_type::relation,
          id: 3,
          role: "subarea".to_string(),
        },
      ],
    },
  );

  let json = to_json(&out);
  assert_eq!(json["tags"]["name"], "Lisboa");
  assert_eq!(
    json["members"],
    serde_json::json!([
      { "type": "n", "id": 1, "role": "admin_centre" },
      { "type": "w", "id": 2, "role": "outer" },
      { "type": "r", "id": 3, "role": "subarea" },
    ])
  );
}

// 00.04: payload de ate 11 bytes cabe no cabecalho de 1 byte
#[test]
fn _00_04_writes_single_byte_header_for_short_payloads() {
  let short = "a".repeat(11);
  assert_eq!(encode_text_object(&short)["tags"]["k"], short);

  let mut out = Vec::new();
  write_header(&mut out, TYPE_TEXTRAW, 11);
  assert_eq!(out, vec![(11u8 << 4) | TYPE_TEXTRAW]);
}

// 00.05: payloads from 12 to 255 bytes use class 12 with 1 extra size byte
#[test]
fn _00_05_writes_two_byte_header_for_payloads_up_to_255() {
  let medium = "b".repeat(255);
  assert_eq!(encode_text_object(&medium)["tags"]["k"], medium);

  let mut out = Vec::new();
  write_header(&mut out, TYPE_TEXTRAW, 255);
  assert_eq!(out, vec![(12u8 << 4) | TYPE_TEXTRAW, 0xFF]);
}

// 00.06: payloads from 256 to 65535 bytes use class 13 with 2 big-endian bytes
#[test]
fn _00_06_writes_three_byte_header_for_payloads_up_to_65535() {
  let large = "c".repeat(65_535);
  assert_eq!(encode_text_object(&large)["tags"]["k"], large);

  let mut out = Vec::new();
  write_header(&mut out, TYPE_TEXTRAW, 65_535);
  assert_eq!(out, vec![(13u8 << 4) | TYPE_TEXTRAW, 0xFF, 0xFF]);
}

// 00.07: payloads above 65535 bytes use class 14 with 4 big-endian bytes
#[test]
fn _00_07_writes_five_byte_header_for_large_payloads() {
  let huge = "d".repeat(70_000);
  assert_eq!(encode_text_object(&huge)["tags"]["k"], huge);

  let mut out = Vec::new();
  write_header(&mut out, TYPE_TEXTRAW, 0xFFFF_FFFF);
  assert_eq!(
    out,
    vec![(14u8 << 4) | TYPE_TEXTRAW, 0xFF, 0xFF, 0xFF, 0xFF]
  );
}

// 00.08: above 2^32 bytes class 15 writes 8 size bytes. a real payload of that
// size is unfeasible, so only the header is checked in isolation
#[test]
fn _00_08_writes_nine_byte_header_for_payloads_above_four_gib() {
  let mut out = Vec::new();
  write_header(&mut out, TYPE_TEXTRAW, 0x1_0000_0000);
  assert_eq!(
    out,
    vec![(15u8 << 4) | TYPE_TEXTRAW, 0, 0, 0, 1, 0, 0, 0, 0]
  );
}

// 00.09: inteiros e floats sao gravados como payload decimal em texto
#[test]
fn _00_09_writes_ints_and_floats_as_decimal_text() {
  let mut out = Vec::new();
  write_int(&mut out, -42);
  assert_eq!(out, vec![(3u8 << 4) | TYPE_INT, b'-', b'4', b'2']);

  let mut out = Vec::new();
  write_float(&mut out, 3.5);
  assert_eq!(out, vec![(3u8 << 4) | TYPE_FLOAT, b'3', b'.', b'5']);
}

// 00.10: o mesmo encoder reutiliza os buffers de scratch entre linhas — o
// resultado precisa ser identico ao de um encoder novo a cada linha
#[test]
fn _00_10_reuses_scratch_buffers_across_encodes() {
  let nodes: Vec<osm_node> = (0..5)
    .map(|i| osm_node {
      id: i,
      lat: i as f64,
      lon: -(i as f64),
      tags: tags(&[("name", "repetido"), ("ref", "x")]),
    })
    .collect();

  let mut shared = encoder::new();
  let reused: Vec<Vec<u8>> = nodes
    .iter()
    .map(|n| {
      let mut out = Vec::new();
      shared.encode_osm_node(&mut out, n);
      out
    })
    .collect();

  let fresh: Vec<Vec<u8>> = nodes
    .iter()
    .map(|n| {
      let mut out = Vec::new();
      encoder::new().encode_osm_node(&mut out, n);
      out
    })
    .collect();

  assert_eq!(
    reused, fresh,
    "reaproveitar scratch nao pode alterar o resultado"
  );
  assert!(
    !shared.scratches.is_empty(),
    "o pool deve reter buffers para reuso"
  );
}
