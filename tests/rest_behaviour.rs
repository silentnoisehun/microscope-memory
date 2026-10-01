//! Behavioural tests for the REST bridge, over a real socket, against the real
//! binary.
//!
//! tests/rest_contract.rs checks that openapi.json and the route table agree.
//! That is necessary and not sufficient: it passed for every commit since
//! a962ad1 removed the `Cmd::Bridge` arm, because the router it describes was
//! unreachable the whole time. A document can match the source perfectly while
//! the source is never started, so these tests start it.
//!
//! Each test spawns `microscope-mem bridge` on loopback with
//! MICROSCOPE_CONFIG pointed at a path that does not exist, which makes
//! Config::load return Config::default() and keeps the test off the developer's
//! real configuration.
//!
//! Requests are written by hand over TcpStream rather than through an HTTP
//! client so that the test needs no dev-dependency, no async runtime, and no
//! feature flags; a request is three lines of text and the response is a status
//! line plus headers.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_microscope-mem");

/// A server process that is killed when it goes out of scope, so a failing
/// assertion cannot leave a listener behind holding the port.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A port that was free a moment ago. Not atomic with the bind, so the caller
/// retries on failure rather than trusting it.
fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("read local addr")
        .port()
}

/// A scratch store the server is started against.
///
/// Config::default() puts output_dir at "./output" relative to the working
/// directory, so the child runs in a directory of its own. The store has to be
/// a real one: an empty `./output` answers 500 on /v1/status, which
/// `status_on_uninitialised_store_is_500` pins down separately, so copying the
/// 8 KB fixture store is what makes the routing tests measure routing.
fn scratch_root() -> PathBuf {
    let dir = std::env::temp_dir().join("microscope-rest-bridge-scratch");
    for sub in ["output", "tmp"] {
        let _ = std::fs::create_dir_all(dir.join(sub));
    }
    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    if let Ok(entries) = std::fs::read_dir(&fixtures) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().ends_with(".bin") || name.to_string_lossy().ends_with(".idx")
            {
                let _ = std::fs::copy(entry.path(), dir.join("output").join(&name));
            }
        }
    }
    dir
}

/// Start the bridge on loopback and wait until it accepts a connection.
///
/// Returns `Err` if it did not come up, so a caller can retry on a different
/// port instead of failing for a reason that is not the code's fault.
fn start(port: u16) -> Result<Server, String> {
    // A path that does not exist: Config::load falls back to the default, so
    // this never reads or writes the developer's own configuration.
    let missing = std::env::temp_dir().join("microscope-rest-bridge-absent.toml");
    let child = Command::new(BIN)
        .args(["bridge", "--host", "127.0.0.1", "--port"])
        .arg(port.to_string())
        .env("MICROSCOPE_CONFIG", missing)
        .current_dir(scratch_root())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn: {e}"))?;

    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().expect("parse addr"),
            Duration::from_millis(200),
        )
        .is_ok()
        {
            return Ok(Server(child));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("bridge did not start listening".to_string())
}

/// Start on any port that works, trying a few before giving up. The port comes
/// back with the server because the tests have to address it.
fn start_any() -> (Server, u16) {
    let mut last = String::new();
    for _ in 0..5 {
        let port = free_port();
        match start(port) {
            Ok(s) => return (s, port),
            Err(e) => last = e,
        }
    }
    panic!("could not start the bridge: {last}");
}

struct Response {
    status: u16,
    headers: Vec<(String, String)>,
}

impl Response {
    fn header(&self, name: &str) -> Option<&str> {
        let want = name.to_ascii_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k == &want)
            .map(|(_, v)| v.as_str())
    }
}

/// Send one request and read the whole response. `Connection: close` makes the
/// server close after responding, so reading to end-of-stream is bounded.
///
/// Written by hand so the test needs no HTTP client dependency, no async
/// runtime and no feature flags.
fn request(port: u16, method: &str, path: &str, extra: &[(&str, &str)]) -> Response {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).expect("connect to bridge");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("set read timeout");

    let mut head = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
    for (k, v) in extra {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).expect("write request");
    stream.flush().expect("flush request");

    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).expect("read response");
    let text = String::from_utf8_lossy(&raw).into_owned();

    let mut lines = text.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    // A reply that is not HTTP at all is a failure, not a status to report.
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("unparseable status line: {status_line:?}"));

    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            break; // end of the header block
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    Response { status, headers }
}

/// The endpoint the startup banner advertises is actually served. This is the
/// failure that went unnoticed for every commit since a962ad1: the banner lives
/// in bridge.rs, so the string is right in the source, but nothing in the
/// binary ever called into the code that prints it.
#[test]
fn openapi_spec_is_served_over_http() {
    let (_server, port) = start_any();
    let r = request(port, "GET", "/openapi.json", &[]);
    assert_eq!(
        r.status, 200,
        "the advertised /openapi.json must be reachable"
    );
    assert!(
        r.header("content-type")
            .is_some_and(|ct| ct.contains("json")),
        "spec must be served as JSON, got {:?}",
        r.header("content-type")
    );
}

/// The routes openapi.json advertises under /v1 answer. This is what the
/// /mobile -> /v1 spec drift in the audit was about, seen from the outside.
#[test]
fn v1_routes_answer() {
    let (_server, port) = start_any();
    for path in ["/v1/status", "/v1/session"] {
        let r = request(port, "GET", path, &[]);
        assert_eq!(r.status, 200, "{path} should answer 200");
    }
}

/// Status codes the audit asked for and no earlier test could see, because a
/// contract test only compares documents against source.
#[test]
fn status_codes_are_correct() {
    let (_server, port) = start_any();

    // Wrong method on a GET-only route. 405, not 404: the route exists.
    let r = request(port, "POST", "/v1/status", &[]);
    assert_eq!(r.status, 405, "POST on a GET route must be 405");

    // A path that does not exist.
    let r = request(port, "GET", "/v1/no-such-route", &[]);
    assert_eq!(r.status, 404, "an unknown path must be 404");

    // A known route missing a required query parameter. 400, not 500.
    let r = request(port, "GET", "/v1/recall", &[]);
    assert_eq!(r.status, 400, "recall without a query is a bad request");

    // The /mobile prefix was documented at the root and answered 404 then; it
    // is not a route now either, and the spec no longer claims it.
    let r = request(port, "GET", "/mobile/feed", &[]);
    assert_eq!(r.status, 404, "/mobile/feed is not a route");
}

/// CORS, observed rather than assumed. With no [server] cors_origin configured
/// the default is a wildcard, and with no api_key set that means any page the
/// developer visits can read a loopback bridge from the browser. Pinning the
/// default down here is what makes a later change to it a visible decision.
#[test]
fn cors_defaults_to_wildcard() {
    let (_server, port) = start_any();
    let r = request(
        port,
        "OPTIONS",
        "/v1/status",
        &[
            ("Origin", "https://claude.ai"),
            ("Access-Control-Request-Method", "GET"),
        ],
    );
    assert_eq!(r.status, 200, "a CORS preflight must be answered");
    assert_eq!(
        r.header("access-control-allow-origin"),
        Some("*"),
        "with no cors_origin configured the default is a wildcard"
    );
}

/// The guard in bridge.rs:run that refuses to expose the bridge on a
/// non-loopback host without an api_key. It is the only thing between a default
/// install and an open listener, so it gets a test rather than a comment.
///
/// No listener involved: the process is expected to exit non-zero before
/// binding anything.
#[test]
fn non_loopback_without_api_key_is_refused() {
    let missing = std::env::temp_dir().join("microscope-rest-bridge-absent.toml");
    let out = Command::new(BIN)
        .args(["bridge", "--host", "0.0.0.0", "--port", "1"])
        .env("MICROSCOPE_CONFIG", missing)
        .output()
        .expect("run bridge on 0.0.0.0");
    assert!(
        !out.status.success(),
        "binding 0.0.0.0 with no api_key must fail, got {:?}",
        out.status
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("refusing to start"),
        "the refusal should say why, got: {stderr}"
    );
}

/// Found while writing the routing test above, and kept as its own test so the
/// behaviour is recorded rather than worked around.
///
/// With no [server] config and no data directory, `/v1/status` answers 500. It
/// is defensible -- the store really is unreadable -- but it means the first
/// request against a freshly cloned checkout that has not been `build`-ed yet
/// is a server error rather than something that says "no data yet". If the
/// status endpoint is meant to report on an uninitialised install, this is the
/// line to change; until then the behaviour is pinned here so it cannot shift
/// unnoticed.
#[test]
fn status_on_uninitialised_store_is_500() {
    let port = free_port();
    let empty = std::env::temp_dir().join("microscope-rest-bridge-empty");
    let _ = std::fs::remove_dir_all(&empty);
    let _ = std::fs::create_dir_all(&empty); // no ./output inside

    let missing = std::env::temp_dir().join("microscope-rest-bridge-absent.toml");
    let child = Command::new(BIN)
        .args(["bridge", "--host", "127.0.0.1", "--port"])
        .arg(port.to_string())
        .env("MICROSCOPE_CONFIG", missing)
        .current_dir(&empty)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn bridge against an empty directory");

    let server = Server(child);
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline && TcpStream::connect(format!("127.0.0.1:{port}")).is_err() {
        std::thread::sleep(Duration::from_millis(100));
    }

    let r = request(port, "GET", "/v1/status", &[]);
    assert_eq!(
        r.status, 500,
        "an uninitialised store currently answers 500, not 200 or 404"
    );
    drop(server);
    let _ = std::fs::remove_dir_all(&empty);
}
