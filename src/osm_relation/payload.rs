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
    enc.write_array(body, |enc, members_body| {
      for member in &relation.members {
        enc.write_object(members_body, |_, member_body| {
          write_text(member_body, "type");
          write_text(member_body, member_type_code(&member.osm_member_type));
          write_text(member_body, "id");
          write_int(member_body, member.id);
          write_text(member_body, "role");
          write_text(member_body, &member.role);
        });
      }
    });
  });
}

fn member_type_code(member_type: &osm_member_type) -> &'static str {
  match member_type {
    osm_member_type::node => "n",
    osm_member_type::way => "w",
    osm_member_type::relation => "r",
  }
}

#[cfg(test)]
#[path = "payload.test.rs"]
mod tests;
