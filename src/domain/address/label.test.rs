use super::{render_friendly_name, validate_friendly_name_format};
use crate::domain::address::fixtures::admin_level_at;

#[test]
fn _00_render_template_resolves_all_placeholders() {
  let admins = vec![
    admin_level_at(2, "brasil"),
    admin_level_at(8, "santos"),
    admin_level_at(12, "rua x"),
  ];
  let out = render_friendly_name(
    "{admin_level_12_name}, {admin_level_8_name}, {admin_level_2_name}",
    &admins,
  );
  assert_eq!(out, "rua x, santos, brasil");
}

#[test]
fn _01_render_template_omits_missing_placeholder_and_collapses_separator() {
  let admins = vec![admin_level_at(2, "brasil"), admin_level_at(12, "rua x")];
  let out = render_friendly_name(
    "{admin_level_12_name}, {admin_level_8_name}, {admin_level_2_name}",
    &admins,
  );
  assert_eq!(out, "rua x, brasil");
}

#[test]
fn _02_validate_format_rejects_unknown_field() {
  let err = validate_friendly_name_format("{foo} {admin_level_12_name}").unwrap_err();
  assert!(err.contains("unknown field 'foo'"), "got: {err}");
}

#[test]
fn _03_render_template_trims_trailing_when_last_placeholder_missing() {
  let admins = vec![admin_level_at(12, "rua x")];
  let out = render_friendly_name("{admin_level_12_name}, {admin_level_8_name}", &admins);
  assert_eq!(out, "rua x");
}

#[test]
fn _04_validate_format_rejects_non_numeric_admin_level() {
  let err = validate_friendly_name_format("{admin_level_x_name}").unwrap_err();
  assert!(err.contains("invalid admin level 'x'"), "got: {err}");
}

#[test]
fn _05_validate_format_rejects_out_of_range_admin_level() {
  let err = validate_friendly_name_format("{admin_level_999_name}").unwrap_err();
  assert!(err.contains("invalid admin level '999'"), "got: {err}");
}

#[test]
fn _06_validate_format_rejects_unterminated_placeholder() {
  let err = validate_friendly_name_format("{admin_level_2_name").unwrap_err();
  assert!(err.contains("unterminated placeholder"), "got: {err}");
}

#[test]
fn _07_validate_format_accepts_valid_template() {
  let format = "{admin_level_12_name}, {admin_level_2_name}";
  assert_eq!(validate_friendly_name_format(format).unwrap(), format);
}

#[test]
fn _08_validate_format_accepts_empty_template() {
  assert_eq!(validate_friendly_name_format("").unwrap(), "");
}

#[test]
fn _09_validate_format_accepts_house_number_alias() {
  assert_eq!(
    validate_friendly_name_format("{house_number}").unwrap(),
    "{house_number}"
  );
}

#[test]
fn _10_render_resolves_house_number_alias() {
  let admins = vec![admin_level_at(12, "rua x"), admin_level_at(30, "123")];
  let out = render_friendly_name("{admin_level_12_name} {house_number}", &admins);
  assert_eq!(out, "rua x 123");
}
