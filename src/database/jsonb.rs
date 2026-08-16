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

pub(crate) const TYPE_INT: u8 = 3;
pub(crate) const TYPE_FLOAT: u8 = 5;
pub(crate) const TYPE_TEXTRAW: u8 = 10;
pub(crate) const TYPE_ARRAY: u8 = 11;
pub(crate) const TYPE_OBJECT: u8 = 12;

pub fn write_header(out: &mut Vec<u8>, jsonb_type: u8, payload_len: usize) {
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

pub fn write_int(out: &mut Vec<u8>, n: i64) {
  let s = n.to_string();
  write_header(out, TYPE_INT, s.len());
  out.extend_from_slice(s.as_bytes());
}

pub fn write_float(out: &mut Vec<u8>, f: f64) {
  let s = format!("{f}");
  write_header(out, TYPE_FLOAT, s.len());
  out.extend_from_slice(s.as_bytes());
}

pub fn write_text(out: &mut Vec<u8>, s: &str) {
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

  // how many scratch buffers the pool is holding. only the reuse test looks at this.
  #[allow(dead_code)]
  pub(crate) fn pooled_buffers(&self) -> usize {
    self.scratches.len()
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

  pub fn write_object<F>(&mut self, out: &mut Vec<u8>, f: F)
  where
    F: FnOnce(&mut encoder, &mut Vec<u8>),
  {
    let mut scratch = self.alloc();
    f(self, &mut scratch);
    write_header(out, TYPE_OBJECT, scratch.len());
    out.extend_from_slice(&scratch);
    self.free(scratch);
  }

  pub fn write_array<F>(&mut self, out: &mut Vec<u8>, f: F)
  where
    F: FnOnce(&mut encoder, &mut Vec<u8>),
  {
    let mut scratch = self.alloc();
    f(self, &mut scratch);
    write_header(out, TYPE_ARRAY, scratch.len());
    out.extend_from_slice(&scratch);
    self.free(scratch);
  }
}

#[cfg(test)]
#[path = "jsonb.test.rs"]
mod tests;
