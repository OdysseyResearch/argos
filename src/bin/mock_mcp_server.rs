//! Tiny MCP server used by integration tests.
//!
//! Reads Content-Length-framed JSON-RPC requests from stdin and replies on
//! stdout with `{ "jsonrpc": "2.0", "id": <id>, "result": { "echo": <params> } }`.
//! For `initialize`, returns `{ "result": { "ok": true } }`.

use std::io::{self, Read, Write};

use serde_json::Value;

fn main() -> io::Result<()> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        // Try to extract a complete frame from `buf`.
        loop {
            let Some(idx) = buf.windows(4).position(|w| w == b"\r\n\r\n") else {
                break;
            };
            let header = match std::str::from_utf8(&buf[..idx]) {
                Ok(s) => s,
                Err(_) => return Ok(()),
            };
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

            let body = buf[idx + 4..total].to_vec();
            buf.drain(..total);

            let parsed: Value = match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let id = parsed.get("id").cloned().unwrap_or(Value::Null);
            let method = parsed.get("method").and_then(Value::as_str).unwrap_or("");
            let params = parsed.get("params").cloned().unwrap_or(Value::Null);

            let result = if method == "initialize" {
                serde_json::json!({"ok": true})
            } else {
                serde_json::json!({"echo": params})
            };
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result,
            });
            let body = serde_json::to_vec(&response)?;
            let header = format!("Content-Length: {}\r\n\r\n", body.len());
            stdout.write_all(header.as_bytes())?;
            stdout.write_all(&body)?;
            stdout.flush()?;
        }

        match stdin.read(&mut chunk) {
            Ok(0) => return Ok(()),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Ok(()),
        }
    }
}
