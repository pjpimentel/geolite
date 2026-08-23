#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

pub const BIN: &str = env!("CARGO_BIN_EXE_geolite");

pub struct scenario {
  pub region: &'static str,
  pub fixture_dir: &'static str,
  pub pbf: &'static str,
  pub preset: &'static str,
}

impl scenario {
  pub fn dir(&self) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("tests")
      .join(self.region)
  }

  pub fn fixture_dir(&self) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("tests")
      .join(self.fixture_dir)
  }

  pub fn city(&self) -> &str {
    self.pbf.split('.').next().unwrap_or(self.pbf)
  }

  pub fn pbf_path(&self) -> PathBuf {
    match std::env::var("GEOLITE_E2E_PBF") {
      Ok(p) => PathBuf::from(p),
      Err(_) => self.fixture_dir().join(self.pbf),
    }
  }
}

pub struct built {
  pub root: PathBuf,
  pub pbf: PathBuf,
  pub data_path: PathBuf,
  pub sqlite_path: PathBuf,
  pub index_path: PathBuf,
  pub stdout: String,
  pub stderr: String,
  pub status: i32,
}

// the pbf is COPIED in and kept outside data_path: the optimize stage deletes every
// *.osm.pbf it finds under data_path, and the committed fixture must never be one of them.
//
// --preset is explicit: the workspace directory is named after the test target, not the map, so
// substring inference on the path would pick the wrong preset for a target like 1_general.
pub fn build(tag: &str, pbf_src: &Path, preset: &str) -> Result<built, String> {
  if !pbf_src.exists() {
    return Err(format!("pbf not found at {}", pbf_src.display()));
  }
  let name = pbf_src
    .file_name()
    .map(|n| n.to_string_lossy().into_owned())
    .unwrap_or_else(|| "source.osm.pbf".to_string());

  let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(tag);
  let data_path = root.join("geolite-store");
  let pbf = root.join("pbf").join(&name);

  let _ = std::fs::remove_dir_all(&root);
  std::fs::create_dir_all(&data_path).map_err(|e| format!("failed to create data dir: {e}"))?;
  std::fs::create_dir_all(pbf.parent().unwrap())
    .map_err(|e| format!("failed to create pbf dir: {e}"))?;
  std::fs::copy(pbf_src, &pbf).map_err(|e| format!("failed to copy the pbf: {e}"))?;

  let out = Command::new(BIN)
    .arg("--data-path")
    .arg(&data_path)
    .arg("--threads")
    .arg("2")
    .arg("--preset")
    .arg(preset)
    .arg("build")
    .arg(&pbf)
    .output()
    .map_err(|e| format!("failed to spawn {BIN}: {e}"))?;

  Ok(built {
    sqlite_path: data_path.join("database.sqlite3"),
    index_path: data_path.join("database.tantivy"),
    stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
    stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    status: out.status.code().unwrap_or(-1),
    root,
    pbf,
    data_path,
  })
}

pub struct world {
  pub scenario: &'static scenario,
  pub root: PathBuf,
  pub pbf: PathBuf,
  pub data_path: PathBuf,
  pub sqlite_path: PathBuf,
  pub index_path: PathBuf,
  pub build_stdout: String,
  pub build_stderr: String,
  pub build_status: i32,
}

// a Result, not a bare world: a panic inside get_or_init leaves the OnceLock empty and the next
// test would pay for the whole build again. recording the failure makes all scenarios fail fast
// with the same message.
pub struct world_cell(OnceLock<Result<world, String>>);

impl world_cell {
  pub const fn new() -> Self {
    Self(OnceLock::new())
  }

  pub fn get(&'static self, scenario: &'static scenario) -> &'static world {
    self
      .0
      .get_or_init(|| build_world(scenario))
      .as_ref()
      .unwrap_or_else(|e| panic!("e2e setup failed: {e}"))
  }
}

fn build_world(scenario: &'static scenario) -> Result<world, String> {
  let pbf_src = scenario.pbf_path();
  if !pbf_src.exists() {
    return Err(format!(
      "fixture not found at {}; regenerate it with tests/{}/{}.build-fixture.sh",
      pbf_src.display(),
      scenario.fixture_dir,
      scenario.city()
    ));
  }

  let bytes = std::fs::read(&pbf_src).map_err(|e| format!("failed to read the fixture: {e}"))?;
  let digest = format!("{:x}", md5::compute(&bytes));
  // keyed on preset + city + content, not on the target: 1_general and 2_preset_brazil build the
  // same santos extract with the same preset, and must share one workspace.
  let tag = format!("{}-{}-{}", scenario.preset, scenario.city(), &digest[..8]);
  drop(bytes);

  let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(&tag);
  let stamp = root.join(".stamp");

  // under coverage the build stages must actually execute: reusing a cached build reports
  // extract/, index/ and optimize/ as 0% simply because an earlier run already did the work.
  let measuring_coverage = std::env::var("LLVM_PROFILE_FILE").is_ok();
  let warm = !measuring_coverage
    && std::env::var("GEOLITE_E2E_REBUILD").is_err()
    && std::fs::read_to_string(&stamp)
      .map(|s| s.trim() == tag)
      .unwrap_or(false);

  if warm {
    let data_path = root.join("geolite-store");
    return Ok(world {
      scenario,
      pbf: root.join("pbf").join(scenario.pbf),
      sqlite_path: data_path.join("database.sqlite3"),
      index_path: data_path.join("database.tantivy"),
      build_stdout: read_or_empty(&root.join("build.stdout")),
      build_stderr: read_or_empty(&root.join("build.stderr")),
      build_status: read_or_empty(&root.join("build.status"))
        .trim()
        .parse()
        .unwrap_or(-1),
      data_path,
      root,
    });
  }

  let built = build(&tag, &pbf_src, scenario.preset)?;

  eprintln!("{}", built.stdout);
  if !built.stderr.is_empty() {
    eprintln!("{}", built.stderr);
  }

  let _ = std::fs::write(built.root.join("build.stdout"), &built.stdout);
  let _ = std::fs::write(built.root.join("build.stderr"), &built.stderr);
  let _ = std::fs::write(built.root.join("build.status"), built.status.to_string());
  if built.status == 0 {
    let _ = std::fs::write(&stamp, &tag);
  }

  Ok(world {
    scenario,
    root: built.root,
    pbf: built.pbf,
    data_path: built.data_path,
    sqlite_path: built.sqlite_path,
    index_path: built.index_path,
    build_stdout: built.stdout,
    build_stderr: built.stderr,
    build_status: built.status,
  })
}

fn read_or_empty(path: &Path) -> String {
  std::fs::read_to_string(path).unwrap_or_default()
}

pub struct output {
  pub status: i32,
  pub stdout: String,
  pub stderr: String,
}

impl world {
  pub fn geolite(&self, args: &[&str]) -> output {
    let out = Command::new(BIN)
      .arg("--data-path")
      .arg(&self.data_path)
      .args(args)
      .output()
      .unwrap_or_else(|e| panic!("failed to spawn {BIN}: {e}"));
    output {
      status: out.status.code().unwrap_or(-1),
      stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
      stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
  }

  pub fn run(&self, args: &[&str]) -> serde_json::Value {
    let mut full = args.to_vec();
    full.extend_from_slice(&["--include-wkt", "false"]);
    self.query_json(&full)
  }

  pub fn query_json(&self, args: &[&str]) -> serde_json::Value {
    let mut full = vec!["query"];
    full.extend_from_slice(args);
    let out = self.geolite(&full);
    assert_eq!(
      out.status, 0,
      "query {args:?} exited {}; stderr: {}",
      out.status, out.stderr
    );
    serde_json::from_str(&out.stdout)
      .unwrap_or_else(|e| panic!("query {args:?} printed invalid json: {e}\n{}", out.stdout))
  }

  pub fn open_sqlite(&self) -> rusqlite::Connection {
    rusqlite::Connection::open_with_flags(
      &self.sqlite_path,
      rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .unwrap_or_else(|e| panic!("failed to open {}: {e}", self.sqlite_path.display()))
  }
}

pub struct response {
  pub status: u16,
  pub headers: Vec<(String, String)>,
  pub body: Vec<u8>,
}

impl response {
  pub fn header(&self, name: &str) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|(k, _)| k.eq_ignore_ascii_case(name))
      .map(|(_, v)| v.as_str())
  }

  pub fn text(&self) -> String {
    String::from_utf8_lossy(&self.body).into_owned()
  }

  pub fn json(&self) -> serde_json::Value {
    serde_json::from_slice(&self.body)
      .unwrap_or_else(|e| panic!("response body is not json: {e}\n{}", self.text()))
  }
}

pub fn request(port: u16, method: &str, path: &str) -> response {
  let addr: SocketAddr = ([127, 0, 0, 1], port).into();
  let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
    .unwrap_or_else(|e| panic!("failed to connect to 127.0.0.1:{port}: {e}"));
  stream.set_read_timeout(Some(Duration::from_secs(30))).ok();

  let req = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
  stream
    .write_all(req.as_bytes())
    .expect("failed to write the request");
  stream.flush().ok();

  let mut raw = Vec::new();
  stream
    .read_to_end(&mut raw)
    .expect("failed to read the response");

  let split = raw
    .windows(4)
    .position(|w| w == b"\r\n\r\n")
    .unwrap_or_else(|| {
      panic!(
        "response has no header terminator: {:?}",
        String::from_utf8_lossy(&raw)
      )
    });
  let head = String::from_utf8_lossy(&raw[..split]).into_owned();
  let body = raw[split + 4..].to_vec();

  let mut lines = head.split("\r\n");
  let status_line = lines.next().unwrap_or_default();
  let status: u16 = status_line
    .split_whitespace()
    .nth(1)
    .and_then(|s| s.parse().ok())
    .unwrap_or_else(|| panic!("malformed status line: {status_line:?}"));

  let headers = lines
    .filter_map(|l| l.split_once(':'))
    .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
    .collect();

  response {
    status,
    headers,
    body,
  }
}

pub fn get(port: u16, path: &str) -> response {
  request(port, "GET", path)
}

pub fn encode(value: &str) -> String {
  let mut out = String::new();
  for b in value.as_bytes() {
    match b {
      b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
      _ => out.push_str(&format!("%{b:02X}")),
    }
  }
  out
}

pub struct server {
  child: Child,
  pub port: u16,
  pub stderr_path: PathBuf,
}

// SIGTERM, not Child::kill (SIGKILL): geolite installs a SIGTERM handler that flips SHUTDOWN and
// lets serve() return, and an instrumented binary only writes its coverage profile on a clean exit.
// killing outright loses every line the server executed.
unsafe extern "C" {
  fn kill(pid: i32, sig: i32) -> i32;
}
const SIGTERM: i32 = 15;

impl Drop for server {
  fn drop(&mut self) {
    unsafe { kill(self.child.id() as i32, SIGTERM) };

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
      match self.child.try_wait() {
        Ok(Some(_)) => return,
        Ok(None) => std::thread::sleep(Duration::from_millis(25)),
        Err(_) => break,
      }
    }
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}

impl server {
  pub fn stderr(&self) -> String {
    read_or_empty(&self.stderr_path)
  }
}

impl world {
  pub fn start_server(&self) -> server {
    self.spawn_server(&self.index_path.to_string_lossy(), "healthy")
  }

  pub fn start_degraded_server(&self) -> server {
    let missing = self.root.join("absent.tantivy");
    self.spawn_server(&missing.to_string_lossy(), "degraded")
  }

  fn spawn_server(&self, index_path: &str, tag: &str) -> server {
    let mut last = String::new();

    // there is no way to hand tiny_http an already-bound socket, so probing a port and letting the
    // child re-bind it is inherently racy. spawn-and-verify closes the race in effect: the child
    // either owns the port or dies, and the loop observes which.
    for attempt in 0..10 {
      let port = probe_free_port();
      let stderr_path = self.root.join(format!("server-{tag}-{attempt}.stderr"));
      let stderr_file = std::fs::File::create(&stderr_path).expect("failed to create the log file");

      let child = Command::new(BIN)
        .arg("--data-path")
        .arg(&self.data_path)
        .arg("--index-path")
        .arg(index_path)
        .arg("http-server")
        .arg("--host")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn {BIN}: {e}"));

      let mut candidate = server {
        child,
        port,
        stderr_path,
      };
      match wait_until_answering(&mut candidate) {
        Ok(()) => return candidate,
        Err(e) => last = e,
      }
    }
    panic!("could not start the http server after 10 attempts: {last}");
  }
}

fn wait_until_answering(candidate: &mut server) -> Result<(), String> {
  let deadline = Instant::now() + Duration::from_secs(60);
  while Instant::now() < deadline {
    if let Ok(Some(status)) = candidate.child.try_wait() {
      return Err(format!(
        "the server exited with {status}: {}",
        candidate.stderr()
      ));
    }
    let port = candidate.port;
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    if TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok()
      && std::panic::catch_unwind(move || get(port, "/status").status).is_ok()
    {
      return Ok(());
    }
    std::thread::sleep(Duration::from_millis(50));
  }
  Err("timed out waiting for the server".to_string())
}

fn probe_free_port() -> u16 {
  TcpListener::bind("127.0.0.1:0")
    .expect("failed to probe a free port")
    .local_addr()
    .expect("failed to read the probe address")
    .port()
}
