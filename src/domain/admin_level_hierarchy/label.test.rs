use super::{nested, root};

#[test]
fn _00_a_root_label_is_the_name_alone() {
  assert_eq!(root("Brasil", None), "Brasil");
}

#[test]
fn _01_a_root_label_carries_its_own_post_code() {
  assert_eq!(root("Santos", Some("11010-000")), "Santos, 11010-000");
}

#[test]
fn _02_a_nested_label_hands_the_parent_label_down() {
  assert_eq!(
    nested("Rua Castro Alves", "Embaré, Santos, São Paulo, Brasil", None),
    "Rua Castro Alves, Embaré, Santos, São Paulo, Brasil"
  );
  assert_eq!(
    nested("Rua Castro Alves", "Embaré, Santos", Some("11025-060")),
    "Rua Castro Alves, Embaré, Santos, 11025-060"
  );
}

#[test]
fn _03_a_blank_post_code_is_ignored() {
  assert_eq!(root("Santos", Some("  ")), "Santos");
  assert_eq!(nested("Rua", "Santos", Some("")), "Rua, Santos");
}
