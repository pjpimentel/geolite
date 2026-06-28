// sqlite JSONB binary encoder. spec: https://sqlite.org/jsonb.html
//
// each element = [header byte][optional extra length bytes][payload].
// header byte = SSSSTTTT where TTTT is type, SSSS is size class:
//   0..=11 : literal payload length
//   12     : 1-byte length follows
//   13     : 2-byte length follows (big-endian)
//   14     : 4-byte length follows
//   15     : 8-byte length follows
//
// types used:
//   3  INT     — text decimal payload ("42")
//   5  FLOAT   — text decimal payload ("3.14")
//   10 TEXTRAW — raw utf-8 bytes (sqlite adds JSON escapes on output)
//   11 ARRAY   — concatenated elements
//   12 OBJECT  — alternating key/value elements

const TYPE_INT: u8 = 3;
const TYPE_FLOAT: u8 = 5;
const TYPE_TEXTRAW: u8 = 10;
const TYPE_ARRAY: u8 = 11;
const TYPE_OBJECT: u8 = 12;

fn write_header(out: &mut Vec<u8>, jsonb_type: u8, payload_len: usize) {
  if payload_len <= 11 {
    out.push(((payload_len as u8) << 4) | jsonb_type);
  } else if payload_len <= 0xFF {
    out.push((12u8 << 4) | jsonb_type);
    out.push(payload_len as u8);
  } else if payload_len <= 0xFFFF {
    out.push((13u8 << 4) | jsonb_type);
    out.extend_from_slice(&(payload_len as u16).to_be_bytes());
  } else if payload_len <= 0xFFFF_FFFF {
    out.push((14u8 << 4) | jsonb_type);
    out.extend_from_slice(&(payload_len as u32).to_be_bytes());
  } else {
    out.push((15u8 << 4) | jsonb_type);
    out.extend_from_slice(&(payload_len as u64).to_be_bytes());
  }
}

fn write_int(out: &mut Vec<u8>, n: i64) {
  let s = n.to_string();
  write_header(out, TYPE_INT, s.len());
  out.extend_from_slice(s.as_bytes());
}

fn write_float(out: &mut Vec<u8>, f: f64) {
  let s = format!("{f}");
  write_header(out, TYPE_FLOAT, s.len());
  out.extend_from_slice(s.as_bytes());
}

fn write_text(out: &mut Vec<u8>, s: &str) {
  let bytes = s.as_bytes();
  write_header(out, TYPE_TEXTRAW, bytes.len());
  out.extend_from_slice(bytes);
}

pub struct encoder {
  // pool of reusable scratch buffers — pop on alloc, push back on free.
  // depth max is 3 (relation > members array > member object), so we usually
  // need 3 scratches active. pool avoids per-row allocation after warmup.
  scratches: Vec<Vec<u8>>,
}

impl encoder {
  pub fn new() -> Self {
    Self {
      scratches: Vec::with_capacity(4),
    }
  }

  fn alloc(&mut self) -> Vec<u8> {
    self
      .scratches
      .pop()
      .unwrap_or_else(|| Vec::with_capacity(256))
  }

  fn free(&mut self, mut buf: Vec<u8>) {
    buf.clear();
    self.scratches.push(buf);
  }

  fn write_object<F>(&mut self, out: &mut Vec<u8>, f: F)
  where
    F: FnOnce(&mut encoder, &mut Vec<u8>),
  {
    let mut scratch = self.alloc();
    f(self, &mut scratch);
    write_header(out, TYPE_OBJECT, scratch.len());
    out.extend_from_slice(&scratch);
    self.free(scratch);
  }

  fn write_array<F>(&mut self, out: &mut Vec<u8>, f: F)
  where
    F: FnOnce(&mut encoder, &mut Vec<u8>),
  {
    let mut scratch = self.alloc();
    f(self, &mut scratch);
    write_header(out, TYPE_ARRAY, scratch.len());
    out.extend_from_slice(&scratch);
    self.free(scratch);
  }

  pub fn encode_osm_node(
    &mut self,
    out: &mut Vec<u8>,
    node: &super::osm_nodes::osm_node,
  ) {
    self.write_object(out, |enc, body| {
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

  pub fn encode_osm_way(
    &mut self,
    out: &mut Vec<u8>,
    way: &super::osm_ways::osm_way,
  ) {
    self.write_object(out, |enc, body| {
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

  pub fn encode_osm_relation(
    &mut self,
    out: &mut Vec<u8>,
    rel: &super::osm_relations::osm_relation,
  ) {
    self.write_object(out, |enc, body| {
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
}

fn member_type_str(t: &super::osm_relations::osm_member_type) -> &'static str {
  use super::osm_relations::osm_member_type;
  match t {
    osm_member_type::node => "n",
    osm_member_type::way => "w",
    osm_member_type::relation => "r",
  }
}
