#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

pub struct stub {
  addr: SocketAddr,
  stopping: Arc<AtomicBool>,
  accepting: Option<JoinHandle<()>>,
}

struct site {
  index_json: String,
  files: Vec<(String, Vec<u8>)>,
}

// `{base}` inside the index json is replaced by the stub's own base url once the port is known.
pub fn start(index_json: &str, files: Vec<(&str, Vec<u8>)>) -> stub {
  let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind the stub");
  let addr = listener
    .local_addr()
    .expect("failed to read the stub address");
  let site = Arc::new(site {
    index_json: index_json.replace("{base}", &format!("http://{addr}")),
    files: files
      .into_iter()
      .map(|(name, bytes)| (name.to_string(), bytes))
      .collect(),
  });
  let stopping = Arc::new(AtomicBool::new(false));
  let flag = Arc::clone(&stopping);
  let accepting = std::thread::spawn(move || {
    for stream in listener.incoming() {
      if flag.load(Ordering::SeqCst) {
        return;
      }
      let Ok(stream) = stream else { continue };
      let site = Arc::clone(&site);
      std::thread::spawn(move || serve(stream, &site));
    }
  });
  stub {
    addr,
    stopping,
    accepting: Some(accepting),
  }
}

impl stub {
  pub fn url(&self, path: &str) -> String {
    format!("http://{}{path}", self.addr)
  }

  pub fn stop(&mut self) {
    self.stopping.store(true, Ordering::SeqCst);
    let _ = TcpStream::connect(self.addr);
    if let Some(accepting) = self.accepting.take() {
      let _ = accepting.join();
    }
  }
}

impl Drop for stub {
  fn drop(&mut self) {
    self.stop();
  }
}

fn serve(mut stream: TcpStream, site: &site) {
  let mut raw = Vec::new();
  let mut buf = [0u8; 1024];
  loop {
    let n = stream.read(&mut buf).unwrap_or(0);
    if n == 0 {
      break;
    }
    raw.extend_from_slice(&buf[..n]);
    if raw.windows(4).any(|w| w == b"\r\n\r\n") {
      break;
    }
  }
  let request = String::from_utf8_lossy(&raw);
  let mut first = request.lines().next().unwrap_or("").split_whitespace();
  let method = first.next().unwrap_or("");
  let name = first
    .next()
    .unwrap_or("")
    .trim_start_matches('/')
    .to_string();
  let range = request
    .lines()
    .find(|l| l.to_ascii_lowercase().starts_with("range:"))
    .and_then(|l| l.split_once(':'))
    .and_then(|(_, v)| parse_range(v.trim()));

  if let Some(base) = name.strip_suffix(".md5") {
    match site.files.iter().find(|(n, _)| n == base) {
      Some((n, bytes)) => reply(
        &mut stream,
        "200 OK",
        format!("{:x}  {n}\n", md5::compute(bytes)).as_bytes(),
      ),
      None => reply(&mut stream, "404 Not Found", b""),
    }
    return;
  }
  if let Some((_, bytes)) = site.files.iter().find(|(n, _)| *n == name) {
    if method == "HEAD" {
      let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n",
        bytes.len()
      );
      stream.write_all(header.as_bytes()).ok();
      return;
    }
    let Some((start, end)) = range else {
      reply(&mut stream, "200 OK", bytes);
      return;
    };
    let last = bytes.len().saturating_sub(1);
    let (start, end) = (start.min(last), end.min(last));
    if bytes.is_empty() || start > end {
      reply(&mut stream, "416 Range Not Satisfiable", b"");
      return;
    }
    let chunk = &bytes[start..=end];
    let header = format!(
      "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
      bytes.len(),
      chunk.len()
    );
    stream.write_all(header.as_bytes()).ok();
    stream.write_all(chunk).ok();
    return;
  }
  reply(&mut stream, "200 OK", site.index_json.as_bytes());
}

fn parse_range(value: &str) -> Option<(usize, usize)> {
  let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
  Some((start.parse().ok()?, end.parse().ok()?))
}

fn reply(stream: &mut TcpStream, status: &str, body: &[u8]) {
  let header = format!(
    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
    body.len()
  );
  stream.write_all(header.as_bytes()).ok();
  stream.write_all(body).ok();
}
