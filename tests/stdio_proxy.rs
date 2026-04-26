//! End-to-end stdio proxy integration test (SC-008, FR-017, FR-008a).
//!
//! Spawns argos-proxy with a mock MCP server (a tiny test binary that echoes
//! frames). Sends `tools/call` and `resources/read` requests across the allow,
//! block, redact, and dry-run paths and asserts:
//!   * upstream receives allowed/redacted calls
//!   * blocked calls return JSON-RPC errors and never reach upstream
//!   * every decision appears in audit.jsonl with a verifiable hash chain
//!
//! The mock MCP server is implemented as a separate test binary in `tests/`.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use argos::audit::{AuditEntry, DecisionLabel, MessageType};
use serde_json::Value;

const ARGOS_BIN: &str = env!("CARGO_BIN_EXE_argos-proxy");

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn write_temp(content: &str, suffix: &str) -> tempfile::NamedTempFile {
    let mut tmp = tempfile::Builder::new()
        .suffix(suffix)
        .tempfile()
        .expect("tempfile");
    tmp.write_all(content.as_bytes()).expect("write");
    tmp.flush().expect("flush");
    tmp
}

/// Frame a JSON-RPC request body with Content-Length wire framing.
fn frame(body: &str) -> Vec<u8> {
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(body.as_bytes());
    out
}

/// Read N Content-Length-framed frames from a reader, blocking up to `timeout`.
fn read_n_frames<R: Read>(reader: &mut R, n: usize, timeout: Duration) -> Vec<Value> {
    let deadline = Instant::now() + timeout;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    let mut frames = Vec::new();

    while frames.len() < n {
        if Instant::now() >= deadline {
            break;
        }
        match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => buf.extend_from_slice(&chunk[..read]),
            Err(_) => break,
        }
        loop {
            let Some(idx) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
                break;
            };
            let header = std::str::from_utf8(&buf[..idx]).unwrap_or("");
            let body_len: usize = header
                .lines()
                .filter_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    if k.trim().eq_ignore_ascii_case("Content-Length") {
                        v.trim().parse().ok()
                    } else {
                        None
                    }
                })
                .next()
                .unwrap_or(0);
            let total = idx + 4 + body_len;
            if buf.len() < total {
                break;
            }
            let body = &buf[idx + 4..total];
            let parsed: Value = serde_json::from_slice(body).expect("valid JSON-RPC frame");
            frames.push(parsed);
            buf.drain(..total);
            if frames.len() >= n {
                break;
            }
        }
    }
    frames
}

fn read_audit(path: &std::path::Path) -> Vec<AuditEntry> {
    let f = std::fs::File::open(path).unwrap();
    BufReader::new(f)
        .lines()
        .filter_map(|l| l.ok())
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<AuditEntry>(&l).unwrap())
        .collect()
}

/// Build a small policy file covering: allow `read_file` with path_prefix
/// constraint, redact `auth_request`, allow `file:///workspace/**` resources,
/// and a default-deny catch-all for tools.
fn standard_policy() -> tempfile::NamedTempFile {
    write_temp(
        r#"
[meta]
version = "0.1"

[[rules]]
tool = "read_file"
action = "allow"
constraints = { path_prefix = "/workspace" }
tags = []

[[rules]]
tool = "auth_request"
action = "redact"
redact = ["token", "password"]
tags = []

[[rules]]
resource = "file:///workspace/**"
action = "allow"
tags = []

[[rules]]
tool = "*"
action = "block"
reason = "default deny"
tags = []
"#,
        ".toml",
    )
}

fn mock_server_path() -> std::path::PathBuf {
    workspace_root().join("target/debug/mock_mcp_server")
}

/// Build the mock server before running tests if needed.
fn ensure_mock_server() {
    let path = mock_server_path();
    if path.is_file() {
        return;
    }
    let status = Command::new("cargo")
        .args(["build", "--bin", "mock_mcp_server"])
        .current_dir(workspace_root())
        .status()
        .expect("cargo build mock_mcp_server");
    assert!(status.success(), "failed to build mock_mcp_server");
}

/// Drive a full proxy session: send `requests` (already framed) on stdin,
/// collect responses, then close stdin and let the proxy drain.
fn run_session(
    policy_path: &std::path::Path,
    audit_path: &std::path::Path,
    agent: &str,
    dry_run: bool,
    requests: &[u8],
    expected_response_count: usize,
) -> Vec<Value> {
    ensure_mock_server();

    let mut cmd = Command::new(ARGOS_BIN);
    cmd.arg("--policy")
        .arg(policy_path)
        .arg("--audit-log")
        .arg(audit_path)
        .arg("--agent")
        .arg(agent);
    if dry_run {
        cmd.arg("--dry-run");
    }
    cmd.arg("--").arg(mock_server_path());
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = cmd.spawn().expect("spawn argos-proxy");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    stdin.write_all(requests).expect("send requests");
    drop(stdin); // close stdin to signal EOF

    let frames = read_n_frames(&mut stdout, expected_response_count, Duration::from_secs(15));

    // Wait for the child to exit cleanly after its stdin closes.
    let _ = child.wait();

    frames
}

#[test]
fn allow_path_reaches_upstream_and_logs() {
    let policy = standard_policy();
    let audit = tempfile::NamedTempFile::new().unwrap();

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "read_file",
            "arguments": {"path": "/workspace/main.rs"}
        }
    });
    let mut input = frame(&req.to_string());
    // Add an unparsed initialize first to ensure pass-through works.
    let init = serde_json::json!({
        "jsonrpc": "2.0", "id": 0, "method": "initialize", "params": {}
    });
    let mut full = frame(&init.to_string());
    full.append(&mut input);

    let responses = run_session(
        policy.path(),
        audit.path(),
        "test-allow",
        false,
        &full,
        2,
    );

    assert_eq!(responses.len(), 2, "expected 2 responses (initialize + tools/call)");

    // The mock server echoes id back. Tools/call should pass through.
    let tools_response = responses.iter().find(|r| r["id"] == 1).expect("tools/call response");
    assert!(
        tools_response.get("result").is_some(),
        "allowed call must produce a result, got: {tools_response}"
    );

    let entries = read_audit(audit.path());
    assert_eq!(entries.len(), 1, "only tools/call should be audited");
    assert_eq!(entries[0].message_type, MessageType::ToolsCall);
    assert_eq!(entries[0].decision, DecisionLabel::Allowed);
    assert_eq!(entries[0].tool_or_resource, "read_file");
    assert_eq!(entries[0].agent, "test-allow");
    assert_eq!(entries[0].sequence, 1);
}

#[test]
fn block_path_returns_jsonrpc_error_and_logs() {
    let policy = standard_policy();
    let audit = tempfile::NamedTempFile::new().unwrap();

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "tools/call",
        "params": {
            "name": "shell_exec",
            "arguments": {"cmd": "rm -rf /"}
        }
    });

    let responses = run_session(
        policy.path(),
        audit.path(),
        "test-block",
        false,
        &frame(&req.to_string()),
        1,
    );

    assert_eq!(responses.len(), 1);
    let resp = &responses[0];
    assert_eq!(resp["id"], 42);
    assert_eq!(resp["error"]["code"], -32000);
    assert!(resp["result"].is_null());

    let entries = read_audit(audit.path());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].decision, DecisionLabel::Blocked);
    assert_eq!(entries[0].tool_or_resource, "shell_exec");
}

#[test]
fn redact_path_strips_fields_before_forwarding() {
    let policy = standard_policy();
    let audit = tempfile::NamedTempFile::new().unwrap();

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "auth_request",
            "arguments": {
                "user": "alice",
                "token": "secret-token-xyz",
                "password": "hunter2"
            }
        }
    });

    let responses = run_session(
        policy.path(),
        audit.path(),
        "test-redact",
        false,
        &frame(&req.to_string()),
        1,
    );

    assert_eq!(responses.len(), 1);
    let resp = &responses[0];
    assert_eq!(resp["id"], 7);
    // Mock server echoes the params it received — verify stripped fields.
    let echoed = &resp["result"]["echo"];
    assert!(
        echoed["arguments"].get("token").is_none(),
        "redact rule must strip token before forwarding"
    );
    assert!(echoed["arguments"].get("password").is_none());
    assert_eq!(echoed["arguments"]["user"], "alice");

    let entries = read_audit(audit.path());
    assert_eq!(entries[0].decision, DecisionLabel::Redacted);
}

#[test]
fn dry_run_passes_blocked_calls_through_with_flag() {
    let policy = standard_policy();
    let audit = tempfile::NamedTempFile::new().unwrap();

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 99,
        "method": "tools/call",
        "params": {
            "name": "shell_exec",
            "arguments": {"cmd": "ls"}
        }
    });

    let responses = run_session(
        policy.path(),
        audit.path(),
        "test-dryrun",
        true,
        &frame(&req.to_string()),
        1,
    );

    assert_eq!(responses.len(), 1);
    // In dry-run, the mock server should receive the call and return a result.
    assert!(
        responses[0]["result"].is_object(),
        "dry-run must let blocked calls through, got: {}",
        responses[0]
    );

    let entries = read_audit(audit.path());
    assert_eq!(entries[0].decision, DecisionLabel::Blocked);
    assert_eq!(entries[0].dry_run, Some(true));
}

#[test]
fn resource_read_allow_and_block_paths() {
    let policy = standard_policy();
    let audit = tempfile::NamedTempFile::new().unwrap();

    let allowed = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "resources/read",
        "params": { "uri": "file:///workspace/src/main.rs" }
    });
    let blocked = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 101,
        "method": "resources/read",
        "params": { "uri": "file:///etc/passwd" }
    });

    let mut input = frame(&allowed.to_string());
    input.extend(frame(&blocked.to_string()));

    let responses = run_session(
        policy.path(),
        audit.path(),
        "test-resources",
        false,
        &input,
        2,
    );

    let allowed_resp = responses.iter().find(|r| r["id"] == 100).unwrap();
    let blocked_resp = responses.iter().find(|r| r["id"] == 101).unwrap();
    assert!(allowed_resp.get("result").is_some());
    assert_eq!(blocked_resp["error"]["code"], -32000);

    let entries = read_audit(audit.path());
    assert_eq!(entries.len(), 2);
    let allowed_entry = entries.iter().find(|e| e.tool_or_resource.contains("main.rs")).unwrap();
    let blocked_entry = entries.iter().find(|e| e.tool_or_resource.contains("passwd")).unwrap();
    assert_eq!(allowed_entry.decision, DecisionLabel::Allowed);
    assert_eq!(allowed_entry.message_type, MessageType::ResourcesRead);
    assert_eq!(blocked_entry.decision, DecisionLabel::Blocked);
}

/// Performance benchmark: assert median round-trip latency through the proxy
/// on the allow path is below 5ms (SC-001). Uses 100 sequential requests as a
/// quick smoke check rather than 1000 to keep CI runtime reasonable.
#[test]
fn allow_path_latency_below_5ms_median() {
    use std::time::Instant;

    let policy = standard_policy();
    let audit = tempfile::NamedTempFile::new().unwrap();

    ensure_mock_server();
    let mut cmd = Command::new(ARGOS_BIN);
    cmd.arg("--policy")
        .arg(policy.path())
        .arg("--audit-log")
        .arg(audit.path())
        .arg("--agent")
        .arg("test-latency")
        .arg("--")
        .arg(mock_server_path());
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    let mut child = cmd.spawn().expect("spawn argos-proxy");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    // Warm-up — first request can include subprocess spawn cost.
    for warm in 0..3 {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": warm,
            "method": "tools/call",
            "params": {"name": "read_file", "arguments": {"path": "/workspace/x"}}
        });
        stdin.write_all(&frame(&req.to_string())).unwrap();
    }
    let _ = read_n_frames(&mut stdout, 3, Duration::from_secs(15));

    const N: usize = 100;
    let mut samples_us = Vec::with_capacity(N);
    for i in 0..N {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": (1000 + i) as u64,
            "method": "tools/call",
            "params": {"name": "read_file", "arguments": {"path": "/workspace/x"}}
        });
        let start = Instant::now();
        stdin.write_all(&frame(&req.to_string())).unwrap();
        let frames = read_n_frames(&mut stdout, 1, Duration::from_secs(15));
        let elapsed = start.elapsed();
        assert_eq!(frames.len(), 1);
        samples_us.push(elapsed.as_micros());
    }

    drop(stdin);
    let _ = child.wait();

    samples_us.sort_unstable();
    let median = samples_us[samples_us.len() / 2];
    let p95 = samples_us[(samples_us.len() * 95) / 100];
    eprintln!(
        "stdio proxy latency: median {} µs, p95 {} µs over {} samples",
        median, p95, samples_us.len()
    );

    assert!(
        median < 5_000,
        "median latency {}µs exceeds SC-001 budget of 5ms",
        median
    );
}

#[test]
fn audit_chain_is_intact_across_session() {
    let policy = standard_policy();
    let audit = tempfile::NamedTempFile::new().unwrap();

    let mut input = Vec::new();
    for i in 0..5 {
        let req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": i,
            "method": "tools/call",
            "params": {
                "name": "read_file",
                "arguments": {"path": "/workspace/x"}
            }
        });
        input.extend(frame(&req.to_string()));
    }

    let _ = run_session(policy.path(), audit.path(), "test-chain", false, &input, 5);

    let entries = read_audit(audit.path());
    assert_eq!(entries.len(), 5);

    const GENESIS: &str =
        "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    assert_eq!(entries[0].prev_hash, GENESIS);
    for i in 1..entries.len() {
        assert_eq!(entries[i].prev_hash, entries[i - 1].entry_hash);
    }
    for (i, entry) in entries.iter().enumerate() {
        let recomputed = argos::audit::writer::compute_entry_hash(entry).unwrap();
        assert_eq!(
            recomputed, entry.entry_hash,
            "entry {i} hash mismatch"
        );
    }
}
