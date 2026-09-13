use super::{
  TYPE_FLOAT, TYPE_INT, TYPE_TEXTRAW, encoder, write_float, write_header, write_int, write_text,
};
use crate::database::jsonb_fixtures::to_json;

fn encode_text_object(value: &str) -> serde_json::Value {
  let mut enc = encoder::new();
  let mut out = Vec::new();
  enc.write_object(&mut out, |_, body| {
    write_text(body, "k");
    write_text(body, value);
  });
  to_json(&out)
}

fn encode_nested(enc: &mut encoder, i: i64) -> Vec<u8> {
  let mut out = Vec::new();
  enc.write_object(&mut out, |enc, body| {
    write_text(body, "id");
    write_int(body, i);
    write_text(body, "tags");
    enc.write_object(body, |_, tags_body| {
      write_text(tags_body, "name");
      write_text(tags_body, "repeated");
    });
    write_text(body, "refs");
    enc.write_array(body, |_, refs_body| {
      write_int(refs_body, i);
      write_int(refs_body, -i);
    });
  });
  out
}

// 00.00: a payload of up to 11 bytes fits the one-byte header
#[test]
fn _00_00_writes_single_byte_header_for_short_payloads() {
  let short = "a".repeat(11);
  assert_eq!(encode_text_object(&short)["k"], short);

  let mut out = Vec::new();
  write_header(&mut out, TYPE_TEXTRAW, 11);
  assert_eq!(out, vec![(11u8 << 4) | TYPE_TEXTRAW]);
}

// 00.01: payloads from 12 to 255 bytes use class 12 with 1 extra size byte
#[test]
fn _00_01_writes_two_byte_header_for_payloads_up_to_255() {
  let medium = "b".repeat(255);
  assert_eq!(encode_text_object(&medium)["k"], medium);

  let mut out = Vec::new();
  write_header(&mut out, TYPE_TEXTRAW, 255);
  assert_eq!(out, vec![(12u8 << 4) | TYPE_TEXTRAW, 0xFF]);
}

// 00.02: payloads from 256 to 65535 bytes use class 13 with 2 big-endian bytes
#[test]
fn _00_02_writes_three_byte_header_for_payloads_up_to_65535() {
  let large = "c".repeat(65_535);
  assert_eq!(encode_text_object(&large)["k"], large);

  let mut out = Vec::new();
  write_header(&mut out, TYPE_TEXTRAW, 65_535);
  assert_eq!(out, vec![(13u8 << 4) | TYPE_TEXTRAW, 0xFF, 0xFF]);
}

// 00.03: payloads above 65535 bytes use class 14 with 4 big-endian bytes
#[test]
fn _00_03_writes_five_byte_header_for_large_payloads() {
  let huge = "d".repeat(70_000);
  assert_eq!(encode_text_object(&huge)["k"], huge);

  let mut out = Vec::new();
  write_header(&mut out, TYPE_TEXTRAW, 0xFFFF_FFFF);
  assert_eq!(
    out,
    vec![(14u8 << 4) | TYPE_TEXTRAW, 0xFF, 0xFF, 0xFF, 0xFF]
  );
}

// 00.04: above 2^32 bytes class 15 writes 8 size bytes. a real payload of that
// size is unfeasible, so only the header is checked in isolation
#[test]
fn _00_04_writes_nine_byte_header_for_payloads_above_four_gib() {
  let mut out = Vec::new();
  write_header(&mut out, TYPE_TEXTRAW, 0x1_0000_0000);
  assert_eq!(
    out,
    vec![(15u8 << 4) | TYPE_TEXTRAW, 0, 0, 0, 1, 0, 0, 0, 0]
  );
}

// 00.05: ints and floats are written as a decimal text payload
#[test]
fn _00_05_writes_ints_and_floats_as_decimal_text() {
  let mut out = Vec::new();
  write_int(&mut out, -42);
  assert_eq!(out, vec![(3u8 << 4) | TYPE_INT, b'-', b'4', b'2']);

  let mut out = Vec::new();
  write_float(&mut out, 3.5);
  assert_eq!(out, vec![(3u8 << 4) | TYPE_FLOAT, b'3', b'.', b'5']);
}

// 00.06: one encoder reuses its scratch buffers across rows; the bytes must match what a fresh
// encoder writes for every row
#[test]
fn _00_06_reuses_scratch_buffers_across_encodes() {
  let mut shared = encoder::new();
  let reused: Vec<Vec<u8>> = (0..5).map(|i| encode_nested(&mut shared, i)).collect();
  let fresh: Vec<Vec<u8>> = (0..5)
    .map(|i| encode_nested(&mut encoder::new(), i))
    .collect();

  assert_eq!(reused, fresh, "reusing scratch buffers must not change the bytes");
  assert!(!shared.scratches.is_empty(), "the pool must keep buffers for reuse");
  assert_eq!(to_json(&reused[3])["refs"], serde_json::json!([3, -3]));
}
