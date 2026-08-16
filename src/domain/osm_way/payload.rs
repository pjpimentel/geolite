// how a way is written into the `payload` column: `{refs, tags}`.
//
// the id is not in the payload — it is the primary key of the row.

use super::entity::osm_way;
use crate::database::jsonb::{encoder, write_int, write_text};

pub fn encode(encoder: &mut encoder, out: &mut Vec<u8>, way: &osm_way) {
  encoder.write_object(out, |enc, body| {
    write_text(body, "refs");
    enc.write_array(body, |_, refs_body| {
      for r in &way.refs {
        write_int(refs_body, *r);
      }
    });
    write_text(body, "tags");
    enc.write_object(body, |_, tags_body| {
      for (k, v) in &way.tags {
        write_text(tags_body, k);
        write_text(tags_body, v);
      }
    });
  });
}

#[cfg(test)]
#[path = "payload.test.rs"]
mod tests;
