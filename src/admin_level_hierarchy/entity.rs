use crate::admin_level::level;

pub struct hierarchy_edges {
  pub admin_level_id: i64,
  pub parents: Vec<i64>,
}

#[derive(Clone)]
pub struct node {
  pub id: i64,
  pub level: level,
  pub name: String,
}
