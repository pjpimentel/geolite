use super::command_handler_http_server;
use crate::extract::pbf_fixtures::tempdir_guard;
use crate::presets::DEFAULT;
use std::io::{Read, Write};
use std::net::TcpStream;

#[test]
fn _00_00_serves_status_from_a_background_thread() {
  let guard = tempdir_guard::new("cli_http_server");
  let db = guard.path.join("db.sqlite3").to_string_lossy().into_owned();
  let index = guard.path.join("idx.tantivy").to_string_lossy().into_owned();
  drop(crate::database::open_write(&db));

  let port = {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("failed to probe a free port");
    listener.local_addr().expect("probe listener addr").port()
  };

  // the server blocks on tiny_http forever, so it runs on a detached thread that dies with the
  // test process; llvm-cov still counts its lines because profiles flush on process exit.
  std::thread::spawn(move || {
    command_handler_http_server(
      &db,
      &index,
      "127.0.0.1",
      port,
      1,
      DEFAULT.index_user_friendly_name.boosts,
    );
  });

  let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
  let response = loop {
    if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
      let request = b"GET /status HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
      if stream.write_all(request).is_ok() {
        let mut buf = String::new();
        let _ = stream.read_to_string(&mut buf);
        if !buf.is_empty() {
          break buf;
        }
      }
    }
    assert!(
      std::time::Instant::now() < deadline,
      "http server did not answer within the deadline"
    );
    std::thread::sleep(std::time::Duration::from_millis(50));
  };
  assert!(response.starts_with("HTTP/1.1"), "unexpected response: {response}");
}
