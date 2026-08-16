// builds a COALESCE(JSON_EXTRACT(...), ...) over a list of osm tags ordered by priority: the
// first one present wins. `payload_expr` is the qualified column expression, e.g.
// `osm_data.osm_ways.payload`. tags must be pre-validated by the cli.
//
// it lives in its own file, with no dependencies of its own, so that the repositories that need it
// do not end up importing the module that creates their tables.
pub fn build_name_select(payload_expr: &str, priority: &[&str]) -> String {
  let parts: Vec<String> = priority
    .iter()
    .map(|tag| format!("JSON_EXTRACT({payload_expr}, '$.tags.\"{tag}\"')"))
    .collect();
  match parts.len() {
    0 => format!("JSON_EXTRACT({payload_expr}, '$.tags.name')"),
    1 => parts.into_iter().next().unwrap(),
    _ => format!("COALESCE({})", parts.join(", ")),
  }
}
