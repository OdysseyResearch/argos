//! End-to-end HTTP/SSE proxy integration test (SC-008).
//!
//! Spins up a mock upstream HTTP server (via axum) on a random port, then
//! starts argos-proxy in HTTP mode pointed at that upstream. Verifies the
//! allow / block / dry-run paths.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use argos::audit::{AuditEntry, DecisionLabel};
use axum::{extract::State, routing::any, Router};
use serde_json::Value;
use tokio::sync::Mutex;

/// SIGTERM the child and wait for it to exit cleanly so the BufWriter inside
/// AuditWriter flushes to disk before we read the log.
fn graceful_stop(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        unsafe {
            libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
        }
        for _ in 0..50 {
            match child.try_wait() {
                Ok(Some(_)) => return,
                _ => std::thread::sleep(Duration::from_millis(50)),
            }
        }
        // Last resort.
        let _ = child.kill();
        let _ = child.wait();
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
        let _ = child.wait();
    }
}

const ARGOS_BIN: &str = env!("CARGO_BIN_EXE_argos-proxy");

fn standard_policy() -> tempfile::NamedTempFile {
    let mut tmp = tempfile::Builder::new().suffix(".toml").tempfile().unwrap();
    tmp.write_all(
        br#"
[meta]
version = "0.1"

[[rules]]
tool = "read_file"
action = "allow"
tags = []

[[rules]]
tool = "*"
action = "block"
reason = "default deny"
tags = []
"#,
    )
    .unwrap();
    tmp.flush().unwrap();
    tmp
}

#[derive(Clone, Default)]
struct UpstreamRecorder {
    received: Arc<Mutex<Vec<Value>>>,
}

async fn upstream_handler(
    State(recorder): State<UpstreamRecorder>,
    body: axum::body::Bytes,
) -> impl axum::response::IntoResponse {
    let parsed: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    recorder.received.lock().await.push(parsed.clone());
    let id = parsed.get("id").cloned().unwrap_or(Value::Null);
    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {"echo": parsed.get("params").cloned().unwrap_or(Value::Null)}
    });
    (
        [(http::header::CONTENT_TYPE, "application/json")],
        response.to_string(),
    )
}

async fn spawn_upstream() -> (String, UpstreamRecorder, tokio::task::JoinHandle<()>) {
    let recorder = UpstreamRecorder::default();

    let app: Router = Router::new()
        .fallback(any(upstream_handler))
        .with_state(recorder.clone());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://127.0.0.1:{port}"), recorder, handle)
}

fn read_audit(path: &std::path::Path) -> Vec<AuditEntry> {
    use std::io::{BufRead, BufReader};
    let f = std::fs::File::open(path).unwrap();
    BufReader::new(f)
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<AuditEntry>(&l).unwrap())
        .collect()
}

/// Find a free TCP port and spawn argos-proxy in HTTP mode pointed at upstream.
async fn start_argos(
    policy_path: &std::path::Path,
    audit_path: &std::path::Path,
    upstream_url: &str,
    dry_run: bool,
) -> (std::process::Child, u16) {
    // Get a free port for argos to bind on.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener); // release so argos can bind it

    let mut cmd = Command::new(ARGOS_BIN);
    cmd.arg("--policy")
        .arg(policy_path)
        .arg("--audit-log")
        .arg(audit_path)
        .arg("--agent")
        .arg("test-http")
        .arg("--upstream")
        .arg(upstream_url)
        .arg("--bind")
        .arg("127.0.0.1")
        .arg("--port")
        .arg(port.to_string());
    if dry_run {
        cmd.arg("--dry-run");
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = cmd.spawn().expect("spawn argos-proxy");

    // Poll TCP-level readiness without making an HTTP request (which would
    // be forwarded to upstream and pollute the recorder).
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // Small grace for axum::serve to be fully ready after listener.bind().
    tokio::time::sleep(Duration::from_millis(50)).await;

    (child, port)
}

#[tokio::test]
async fn http_allow_path_forwards_to_upstream() {
    let (upstream_url, recorder, _upstream) = spawn_upstream().await;
    let policy = standard_policy();
    let audit = tempfile::NamedTempFile::new().unwrap();

    let (mut child, port) =
        start_argos(policy.path(), audit.path(), &upstream_url, false).await;

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": "read_file", "arguments": {"path": "/x"}}
    });

    let resp = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/"))
        .header("content-type", "application/json")
        .body(req.to_string())
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["id"], 1);
    assert!(body.get("result").is_some());

    let received = recorder.received.lock().await;
    assert_eq!(received.len(), 1, "upstream must receive allowed call");
    drop(received);

    graceful_stop(&mut child);

    let entries = read_audit(audit.path());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].decision, DecisionLabel::Allowed);
}

#[tokio::test]
async fn http_block_path_returns_jsonrpc_error_no_upstream_call() {
    let (upstream_url, recorder, _upstream) = spawn_upstream().await;
    let policy = standard_policy();
    let audit = tempfile::NamedTempFile::new().unwrap();

    let (mut child, port) =
        start_argos(policy.path(), audit.path(), &upstream_url, false).await;

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 99,
        "method": "tools/call",
        "params": {"name": "shell_exec", "arguments": {"cmd": "rm"}}
    });

    let resp = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/"))
        .header("content-type", "application/json")
        .body(req.to_string())
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], -32000);

    let received = recorder.received.lock().await;
    assert!(
        received.is_empty(),
        "blocked call must not reach upstream, got: {received:?}"
    );

    graceful_stop(&mut child);

    let entries = read_audit(audit.path());
    assert_eq!(entries[0].decision, DecisionLabel::Blocked);
}

#[tokio::test]
async fn http_dry_run_passes_blocked_call_through_with_flag() {
    let (upstream_url, recorder, _upstream) = spawn_upstream().await;
    let policy = standard_policy();
    let audit = tempfile::NamedTempFile::new().unwrap();

    let (mut child, port) =
        start_argos(policy.path(), audit.path(), &upstream_url, true).await;

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {"name": "shell_exec", "arguments": {}}
    });

    let resp = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{port}/"))
        .header("content-type", "application/json")
        .body(req.to_string())
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert!(body.get("result").is_some(), "dry-run must forward, got: {body}");

    let received = recorder.received.lock().await;
    assert_eq!(received.len(), 1);

    graceful_stop(&mut child);

    let entries = read_audit(audit.path());
    assert_eq!(entries[0].decision, DecisionLabel::Blocked);
    assert_eq!(entries[0].dry_run, Some(true));
}
