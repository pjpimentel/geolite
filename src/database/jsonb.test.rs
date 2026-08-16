use super::*;

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
