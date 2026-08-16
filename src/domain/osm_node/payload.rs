use super::entity::osm_node;
use crate::database::jsonb::{encoder, write_float, write_text};

pub fn encode(encoder: &mut encoder, out: &mut Vec<u8>, node: &osm_node) {
  encoder.write_object(out, |enc, body| {
    write_text(body, "lat");
    write_float(body, node.lat);
    write_text(body, "lon");
    write_float(body, node.lon);
    write_text(body, "tags");
    enc.write_object(body, |_, tags_body| {
      for (k, v) in &node.tags {
        write_text(tags_body, k);
        write_text(tags_body, v);
      }
    });
  });
}

#[cfg(test)]
#[path = "payload.test.rs"]
mod tests;
