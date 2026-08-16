use crate::database::jsonb::{encoder, write_int, write_text};

use super::osm_relations::{osm_member_type, osm_relation};
use super::osm_ways::osm_way;

// how a way and a relation are written into their `payload` column. they stay in the pipeline
// until each gets its own slice, next to the decoders they pair with.

pub fn encode_way(encoder: &mut encoder, out: &mut Vec<u8>, way: &osm_way) {
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

pub fn encode_relation(encoder: &mut encoder, out: &mut Vec<u8>, rel: &osm_relation) {
  encoder.write_object(out, |enc, body| {
      write_text(body, "tags");
      enc.write_object(body, |_, tags_body| {
        for (k, v) in &rel.tags {
          write_text(tags_body, k);
          write_text(tags_body, v);
        }
      });
      write_text(body, "members");
      enc.write_array(body, |enc2, members_body| {
        for m in &rel.members {
          enc2.write_object(members_body, |_, mem_body| {
            write_text(mem_body, "type");
            write_text(mem_body, member_type_str(&m.osm_member_type));
            write_text(mem_body, "id");
            write_int(mem_body, m.id);
            write_text(mem_body, "role");
            write_text(mem_body, &m.role);
          });
        }
      });
  });
}

fn member_type_str(t: &osm_member_type) -> &'static str {
  match t {
    osm_member_type::node => "n",
    osm_member_type::way => "w",
    osm_member_type::relation => "r",
  }
}

#[cfg(test)]
#[path = "element_payload.test.rs"]
mod tests;
