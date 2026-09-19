use crate::admin_level::level;

pub struct hierarchy_edges {
  pub admin_level_id: i64,
  pub parents: Vec<i64>,
}

// read by the directory view only, which the tui and the places api of the backlog will bring
#[allow(dead_code)]
pub struct node {
  pub id: i64,
  pub level: level,
  pub name: String,
}
