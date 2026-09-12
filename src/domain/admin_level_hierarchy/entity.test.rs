use super::{decode_chain, encode_chain};

#[test]
fn _00_a_chain_round_trips_through_json() {
  let chain = vec![15, 3, 1];
  assert_eq!(encode_chain(&chain), "[15,3,1]");
  assert_eq!(decode_chain(&encode_chain(&chain)), chain);
}

#[test]
fn _01_an_empty_chain_is_an_empty_array() {
  assert_eq!(encode_chain(&[]), "[]");
  assert!(decode_chain("[]").is_empty());
}

#[test]
fn _02_unreadable_json_decodes_to_an_empty_chain() {
  assert!(decode_chain("not json").is_empty());
  assert!(decode_chain("").is_empty());
}
