use super::entity::{osm_member_type, osm_relation};
use crate::database::jsonb::{encoder, write_int, write_text};

pub fn encode(encoder: &mut encoder, out: &mut Vec<u8>, relation: &osm_relation) {
  encoder.write_object(out, |enc, body| {
    write_text(body, "tags");
    enc.write_object(body, |_, tags_body| {
      for (k, v) in &relation.tags {
        write_text(tags_body, k);
        write_text(tags_body, v);
      }
    });
    write_text(body, "members");
    enc.write_array(body, |enc2, members_body| {
      for m in &relation.members {
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
#[path = "payload.test.rs"]
mod tests;
