use super::*;

use crate::database::jsonb::{TYPE_TEXTRAW, encoder, write_header};
use crate::domain::osm_node::osm_node;

// sqlite itself is the oracle for the format: a malformed jsonb makes JSON() fail or return
// something other than what was written.
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
  encode(
    &mut enc,
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
  encode(

    &mut enc,
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
  encode(

    &mut enc,
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
      encode(&mut shared, &mut out, n);
      out
    })
    .collect();

  let fresh: Vec<Vec<u8>> = nodes
    .iter()
    .map(|n| {
      let mut out = Vec::new();
      encode(&mut encoder::new(), &mut out, n);
      out
    })
    .collect();

  assert_eq!(
    reused, fresh,
    "reaproveitar scratch nao pode alterar o resultado"
  );
  assert!(
    shared.pooled_buffers() > 0,
    "o pool deve reter buffers para reuso"
  );
}
