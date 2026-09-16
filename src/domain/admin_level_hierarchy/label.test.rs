use super::render;

#[test]
fn _00_a_root_label_is_the_name_alone() {
  assert_eq!(render(("Brasil", None), std::iter::empty()), "Brasil");
  assert_eq!(render(("Santos", Some("11010-000")), std::iter::empty()), "Santos, 11010-000");
}

#[test]
fn _01_the_names_go_outward_and_the_post_codes_come_back_inward() {
  assert_eq!(
    render(
      ("Rua Castro Alves", Some("11025-060")),
      [("Embaré", None), ("Santos", Some("11000-000")), ("Brasil", None)].into_iter()
    ),
    "Rua Castro Alves, Embaré, Santos, Brasil, 11000-000, 11025-060"
  );
}

#[test]
fn _02_a_blank_post_code_is_ignored() {
  assert_eq!(
    render(("Rua", Some("")), [("Santos", Some("  "))].into_iter()),
    "Rua, Santos"
  );
}
