#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const BIN: &str = env!("CARGO_BIN_EXE_geolite");

pub struct scenario {
  pub region: &'static str,
  pub fixture_dir: &'static str,
  pub pbf: &'static str,
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
pub fn build(tag: &str, pbf_src: &Path) -> Result<built, String> {
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

  remove_sealed_dir(&root);
  std::fs::create_dir_all(&data_path).map_err(|e| format!("failed to create data dir: {e}"))?;
  std::fs::create_dir_all(pbf.parent().unwrap())
    .map_err(|e| format!("failed to create pbf dir: {e}"))?;
  std::fs::copy(pbf_src, &pbf).map_err(|e| format!("failed to copy the pbf: {e}"))?;

  let out = Command::new(BIN)
    .arg("--data-path")
    .arg(&data_path)
    .arg("--threads")
    .arg("2")
    .arg("build")
    .arg(&pbf)
    .output()
    .map_err(|e| format!("failed to spawn {BIN}: {e}"))?;

  Ok(built {
    sqlite_path: data_path.join("br/data.sqlite3"),
    index_path: data_path.join("br/index.tantivy"),
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
  // keyed on city + content, not on the target: 01_general and 02_preset_brazil build the same
  // santos extract, and must share one workspace.
  let tag = format!("{}-{}", scenario.city(), &digest[..8]);
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
      sqlite_path: data_path.join("br/data.sqlite3"),
      index_path: data_path.join("br/index.tantivy"),
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

  let built = build(&tag, &pbf_src)?;

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
    self.geolite_in(&self.data_path, args)
  }

  pub fn scratch(&self, name: &str) -> PathBuf {
    let dir = self.root.join("scratch").join(name);
    remove_sealed_dir(&dir);
    std::fs::create_dir_all(&dir)
      .unwrap_or_else(|e| panic!("failed to create {}: {e}", dir.display()));
    dir
  }

  pub fn geolite_in(&self, data_path: &Path, args: &[&str]) -> output {
    let out = Command::new(BIN)
      .arg("--data-path")
      .arg(data_path)
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

  // the world read as one database: its base with the catalogue attached
  pub fn open_sqlite(&self) -> rusqlite::Connection {
    open_scratch(&self.data_path)
  }
}

pub fn open_sqlite_at(path: &Path) -> rusqlite::Connection {
  rusqlite::Connection::open_with_flags(
    path,
    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
  )
  .unwrap_or_else(|e| panic!("failed to open {}: {e}", path.display()))
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
  request_with(port, method, path, &[])
}

pub fn request_with(port: u16, method: &str, path: &str, headers: &[(&str, &str)]) -> response {
  let addr: SocketAddr = ([127, 0, 0, 1], port).into();
  let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
    .unwrap_or_else(|e| panic!("failed to connect to 127.0.0.1:{port}: {e}"));
  stream.set_read_timeout(Some(Duration::from_secs(30))).ok();

  let mut req = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
  for (name, value) in headers {
    req.push_str(&format!("{name}: {value}\r\n"));
  }
  req.push_str("\r\n");
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

pub fn get_with(port: u16, path: &str, headers: &[(&str, &str)]) -> response {
  request_with(port, "GET", path, headers)
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
  pub stdout_path: PathBuf,
  pub stderr_path: PathBuf,
}

// every spawn gets its own log files: the world is shared by every test of the process
static SPAWN_SEQ: AtomicU64 = AtomicU64::new(0);

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
  pub fn stdout(&self) -> String {
    read_or_empty(&self.stdout_path)
  }

  pub fn stderr(&self) -> String {
    read_or_empty(&self.stderr_path)
  }
}

impl world {
  pub fn start_server(&self) -> server {
    self.spawn_server_at(&self.data_path, None, "healthy")
  }

  // the one base of the world, named, with an index path that holds nothing
  pub fn start_degraded_server(&self) -> server {
    let missing = self.root.join("absent.tantivy");
    self.spawn_server_at(
      &self.data_path,
      Some((&self.sqlite_path, &missing)),
      "degraded",
    )
  }

  // a server over another data path, a copy of the world relabelled to another country or a
  // second base beside the first for instance
  pub fn start_server_at(&self, data_path: &Path) -> server {
    self.spawn_server_at(data_path, None, "copy")
  }

  fn spawn_server_at(&self, data_path: &Path, single: Option<(&Path, &Path)>, tag: &str) -> server {
    // the os picks the port (`--port 0`) and the child names it on stdout: nothing to probe
    let seq = SPAWN_SEQ.fetch_add(1, Ordering::Relaxed);
    let stdout_path = self.root.join(format!("server-{tag}-{seq}.stdout"));
    let stderr_path = self.root.join(format!("server-{tag}-{seq}.stderr"));
    let stdout_file = std::fs::File::create(&stdout_path).expect("failed to create the stdout log");
    let stderr_file = std::fs::File::create(&stderr_path).expect("failed to create the stderr log");

    let mut command = Command::new(BIN);
    command.arg("--data-path").arg(data_path);
    if let Some((sqlite_path, index_path)) = single {
      command
        .arg("--sqlite-path")
        .arg(sqlite_path)
        .arg("--index-path")
        .arg(index_path);
    }
    let child = command
      .arg("http-server")
      .arg("--host")
      .arg("127.0.0.1")
      .arg("--port")
      .arg("0")
      .stdout(Stdio::from(stdout_file))
      .stderr(Stdio::from(stderr_file))
      .spawn()
      .unwrap_or_else(|e| panic!("failed to spawn {BIN}: {e}"));

    let mut candidate = server {
      child,
      port: 0,
      stdout_path,
      stderr_path,
    };
    candidate.port = wait_for_port(&mut candidate)
      .unwrap_or_else(|e| panic!("could not start the http server: {e}"));
    wait_until_answering(&mut candidate)
      .unwrap_or_else(|e| panic!("could not start the http server: {e}"));
    candidate
  }
}

// the port is on the `listening on 127.0.0.1:PORT ...` line `http::serve` prints; a reworded line
// shows up here as a timeout
fn wait_for_port(candidate: &mut server) -> Result<u16, String> {
  let deadline = Instant::now() + Duration::from_secs(60);
  while Instant::now() < deadline {
    if let Ok(Some(status)) = candidate.child.try_wait() {
      return Err(format!(
        "the server exited with {status}: {}",
        candidate.stderr()
      ));
    }
    let stdout = plain(&candidate.stdout());
    let port = stdout
      .split_inclusive('\n')
      .filter(|line| line.ends_with('\n'))
      .find_map(|line| line.strip_prefix("listening on 127.0.0.1:"))
      .and_then(|rest| rest.split_whitespace().next())
      .and_then(|digits| digits.parse::<u16>().ok());
    if let Some(port) = port {
      return Ok(port);
    }
    std::thread::sleep(Duration::from_millis(50));
  }
  Err(format!(
    "timed out waiting for the listening line in {} (stderr: {})",
    candidate.stdout_path.display(),
    candidate.stderr()
  ))
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

// the stages colour their verbs with ansi escapes; the words are asserted without them
pub fn plain(text: &str) -> String {
  let mut out = String::with_capacity(text.len());
  let mut rest = text;
  while let Some(start) = rest.find('\x1b') {
    out.push_str(&rest[..start]);
    rest = match rest[start..].find('m') {
      Some(end) => &rest[start + end + 1..],
      None => "",
    };
  }
  out.push_str(rest);
  out
}

// every line is found after the one before it, so the text names them all and in this order
pub fn assert_in_order(text: &str, lines: &[&str]) {
  let mut at = 0;
  for line in lines {
    let found = text[at..]
      .find(line)
      .unwrap_or_else(|| panic!("{line:?} is missing or out of order:\n{text}"));
    at += found + line.len();
  }
}

// a text query against the database of a scratch or of the world
pub fn query_at(w: &world, data_path: &Path, text: &str) -> serde_json::Value {
  let out = w.geolite_in(data_path, &["query", text, "--include-wkt", "false"]);
  assert_eq!(
    out.status, 0,
    "query {text:?} exited {}:\n{}",
    out.status, out.stderr
  );
  serde_json::from_str(&out.stdout).expect("the query must print json")
}

// the preset of a row is its country's: a base answers under another country, or under the
// default preset, once every row is relabelled
pub fn relabel_base(sqlite_path: &Path, iso: Option<&str>) {
  const SQL_RELABEL_COUNTRY: &str = "
    UPDATE admin_levels
    SET country_iso_code = ?1
  ";

  let conn = rusqlite::Connection::open(sqlite_path)
    .unwrap_or_else(|e| panic!("failed to open {}: {e}", sqlite_path.display()));
  conn
    .execute(SQL_RELABEL_COUNTRY, [iso])
    .expect("failed to relabel the country of every row");
}

pub fn relabel_country(data_path: &Path, iso: Option<&str>) {
  relabel_base(&data_path.join("br/data.sqlite3"), iso);
}

// a scratch read as one database: its brazilian base, or the catalogue when no base was written
// yet, with the catalogue attached and its ledger reachable as `osm_pbf_files`
pub fn open_scratch(dir: &Path) -> rusqlite::Connection {
  const SQL_LEDGER_VIEW: &str = "
    CREATE TEMP VIEW osm_pbf_files AS
    SELECT *
    FROM catalog.osm_pbf_files
  ";

  let base = dir.join("br/data.sqlite3");
  let catalog = dir.join("catalog.sqlite3");
  let main = if base.exists() { &base } else { &catalog };
  let conn = rusqlite::Connection::open(main)
    .unwrap_or_else(|e| panic!("failed to open {}: {e}", main.display()));
  if base.exists() && catalog.exists() {
    conn
      .execute(
        "ATTACH DATABASE ?1 AS catalog",
        [catalog.to_string_lossy().as_ref()],
      )
      .expect("failed to attach the catalogue");
    conn
      .execute_batch(SQL_LEDGER_VIEW)
      .expect("failed to view the ledger");
  }
  conn
}

impl world {
  // a copy of the world's base and index, to be relabelled or reindexed without touching the world
  pub fn world_copy(&self, name: &str) -> PathBuf {
    let dir = self.scratch(name);
    self.copy_base_into(&dir, "br");
    dir
  }

  // the world's base copied beside a scratch's bases under another country, every row relabelled:
  // the way a data path holds two countries
  pub fn copy_base_into(&self, dir: &Path, iso: &str) {
    let folder = dir.join(iso);
    std::fs::create_dir_all(&folder)
      .unwrap_or_else(|e| panic!("failed to create {}: {e}", folder.display()));
    let sqlite = folder.join("data.sqlite3");
    std::fs::copy(&self.sqlite_path, &sqlite)
      .unwrap_or_else(|e| panic!("failed to copy the base: {e}"));
    make_writable(&sqlite);
    copy_dir(&self.index_path, &folder.join("index.tantivy"));
    if iso != "br" {
      relabel_base(&sqlite, Some(&iso.to_ascii_uppercase()));
    }
  }
}

fn copy_dir(from: &Path, to: &Path) {
  std::fs::create_dir_all(to).unwrap_or_else(|e| panic!("failed to create {}: {e}", to.display()));
  for entry in
    std::fs::read_dir(from).unwrap_or_else(|e| panic!("failed to read {}: {e}", from.display()))
  {
    let entry = entry.expect("failed to read a directory entry");
    let target = to.join(entry.file_name());
    if entry.path().is_dir() {
      copy_dir(&entry.path(), &target);
    } else {
      std::fs::copy(entry.path(), &target)
        .unwrap_or_else(|e| panic!("failed to copy {}: {e}", entry.path().display()));
      make_writable(&target);
    }
  }
}

// a build seals what it leaves behind, and a sealed directory cannot be emptied: the write bits
// come back before a workspace is removed
pub fn remove_sealed_dir(path: &Path) {
  use std::os::unix::fs::PermissionsExt;

  fn lift(path: &Path) {
    let Ok(metadata) = std::fs::metadata(path) else {
      return;
    };
    let mode = if metadata.is_dir() { 0o755 } else { 0o644 };
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    if metadata.is_dir() {
      for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
        lift(&entry.path());
      }
    }
  }

  lift(path);
  let _ = std::fs::remove_dir_all(path);
}

// a copy of a file the build sealed is the test's to change
pub fn make_writable(path: &Path) {
  use std::os::unix::fs::PermissionsExt;

  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))
    .unwrap_or_else(|e| panic!("failed to make {} writable: {e}", path.display()));
}

// the wkb column as it is stored: spatialite's blob layout
pub fn decode_wkb(blob: &[u8]) -> geo::Geometry<f64> {
  use geozero::ToGeo;
  geozero::wkb::SpatiaLiteWkb(blob)
    .to_geo()
    .unwrap_or_else(|e| panic!("undecodable geometry: {e}"))
}

// the merged_way_ids column as it is stored: a jsonb array, read back as text
pub fn merged_way_ids_of(conn: &rusqlite::Connection, id: i64) -> Option<Vec<u64>> {
  let text: Option<String> = conn
    .query_row(
      "SELECT JSON(merged_way_ids) FROM admin_levels WHERE id = ?1",
      [id],
      |r| r.get(0),
    )
    .unwrap_or_else(|e| panic!("no admin_levels row {id}: {e}"));
  text.map(|text| serde_json::from_str(&text).expect("merged_way_ids is not a json array of ids"))
}
