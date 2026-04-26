//! stdio transport: Content-Length codec, subprocess spawn/forward, graceful shutdown.

use std::process::Stdio;
use std::sync::Arc;

use bytes::{Buf, Bytes, BytesMut};
use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio_util::codec::{Decoder, Encoder};
use tokio_util::sync::CancellationToken;

use crate::audit::AuditWriter;
use crate::error::AppError;
use crate::policy::PolicyEngine;
use crate::proxy::{intercept, InterceptOutcome};
use crate::transport::{McpFrame, SessionConfig};

const HEADER_TERMINATOR: &[u8] = b"\r\n\r\n";

/// MCP/LSP wire-protocol framing: `Content-Length: <N>\r\n\r\n<JSON-bytes>` (FR-017).
#[derive(Debug, Default)]
pub(crate) struct ContentLengthCodec {
    state: DecoderState,
}

#[derive(Debug, Default)]
enum DecoderState {
    #[default]
    ReadingHeader,
    ReadingBody {
        body_len: usize,
    },
}

impl Decoder for ContentLengthCodec {
    type Item = McpFrame;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        loop {
            match self.state {
                DecoderState::ReadingHeader => {
                    let Some(idx) = find_subsequence(src, HEADER_TERMINATOR) else {
                        return Ok(None);
                    };
                    let header_bytes = src.split_to(idx + HEADER_TERMINATOR.len());
                    let header_str = std::str::from_utf8(&header_bytes[..idx])
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

                    let body_len = parse_content_length(header_str).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "missing or invalid Content-Length header",
                        )
                    })?;

                    self.state = DecoderState::ReadingBody { body_len };
                }
                DecoderState::ReadingBody { body_len } => {
                    if src.len() < body_len {
                        src.reserve(body_len - src.len());
                        return Ok(None);
                    }
                    let body = src.split_to(body_len).freeze();
                    self.state = DecoderState::ReadingHeader;
                    return Ok(Some(McpFrame { body }));
                }
            }
        }
    }
}

impl Encoder<&[u8]> for ContentLengthCodec {
    type Error = io::Error;

    fn encode(&mut self, body: &[u8], dst: &mut BytesMut) -> Result<(), Self::Error> {
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        dst.reserve(header.len() + body.len());
        dst.extend_from_slice(header.as_bytes());
        dst.extend_from_slice(body);
        Ok(())
    }
}

impl Encoder<Bytes> for ContentLengthCodec {
    type Error = io::Error;

    fn encode(&mut self, body: Bytes, dst: &mut BytesMut) -> Result<(), Self::Error> {
        Encoder::<&[u8]>::encode(self, body.chunk(), dst)
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_content_length(header: &str) -> Option<usize> {
    for line in header.split("\r\n") {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (key, value) = line.split_once(':')?;
        if key.trim().eq_ignore_ascii_case("Content-Length") {
            return value.trim().parse::<usize>().ok();
        }
    }
    None
}

/// Encode a single frame into a Content-Length-framed byte buffer.
pub(crate) fn frame_to_wire_bytes(body: &[u8]) -> Vec<u8> {
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    let mut out = Vec::with_capacity(header.len() + body.len());
    out.extend_from_slice(header.as_bytes());
    out.extend_from_slice(body);
    out
}

/// Spawn the upstream MCP server as a child process and proxy the stdio
/// channel between the MCP client and the child, intercepting policy-relevant
/// methods and writing audit entries (FR-016, FR-018, FR-018c, FR-028).
pub async fn run_stdio_proxy(
    server_command: &[String],
    engine: Arc<PolicyEngine>,
    audit: Arc<AuditWriter>,
    session_id: String,
    config: Arc<SessionConfig>,
) -> Result<(), AppError> {
    if server_command.is_empty() {
        return Err(AppError::Upstream("empty server command".to_string()));
    }
    let mut command = Command::new(&server_command[0]);
    if server_command.len() > 1 {
        command.args(&server_command[1..]);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    let mut child: Child = command
        .spawn()
        .map_err(|e| AppError::Upstream(format!("failed to spawn upstream server: {e}")))?;

    let child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Upstream("upstream stdin missing".into()))?;
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Upstream("upstream stdout missing".into()))?;

    let cancel = CancellationToken::new();
    let to_child = Arc::new(Mutex::new(child_stdin));
    let to_client = Arc::new(Mutex::new(tokio::io::stdout()));
    let session_id = Arc::new(session_id);

    // Client → proxy → child: intercept on this direction. When client stdin
    // closes (EOF), this task closes the child's stdin (so the child sees
    // EOF and can exit cleanly) but does NOT cancel — `child_to_client` keeps
    // running until the child's stdout closes, draining any final responses.
    let client_to_child = {
        let engine = engine.clone();
        let audit = audit.clone();
        let session_id = session_id.clone();
        let config = config.clone();
        let to_child_for_pump = to_child.clone();
        let to_client = to_client.clone();
        let cancel = cancel.clone();

        tokio::spawn(async move {
            let stdin = tokio::io::stdin();
            let result = pump_with_intercept(
                stdin,
                to_child_for_pump,
                to_client,
                engine,
                audit,
                session_id,
                config,
                cancel.clone(),
            )
            .await;
            // Close child's stdin so it sees EOF and shuts down cleanly.
            // We hold the mutex briefly and shut down the underlying handle.
            let mut guard = to_child.lock().await;
            let _ = guard.shutdown().await;
            result
        })
    };

    // Child → proxy → client: pure forwarding, no audit (responses).
    let child_to_client = {
        let to_client = to_client.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move { pump_raw(child_stdout, to_client, cancel).await })
    };

    // Wait for shutdown trigger or natural completion of the child process.
    let shutdown_signal = wait_for_shutdown_signal(cancel.clone());

    let upstream_status = tokio::select! {
        _ = shutdown_signal => {
            // SIGTERM/SIGINT received — drain in-flight tasks (FR-018c).
            cancel.cancel();
            None
        }
        res = child.wait() => {
            // Cancel after a brief grace period to let child_to_client drain
            // any final response bytes from the now-closed child stdout.
            Some(res)
        }
    };

    // Drain in-flight forwarders.
    let _ = client_to_child.await;
    cancel.cancel();
    let _ = child_to_client.await;
    audit.flush().await?;

    if let Some(res) = upstream_status {
        match res {
            Ok(status) if status.success() => {}
            Ok(status) => {
                return Err(AppError::Upstream(format!(
                    "upstream subprocess exited with status {status}"
                )));
            }
            Err(e) => return Err(AppError::Upstream(format!("upstream wait failed: {e}"))),
        }
    }
    Ok(())
}

/// Wait for SIGTERM or SIGINT. Resolves when either signal is received or the
/// cancel token is triggered from another task.
async fn wait_for_shutdown_signal(cancel: CancellationToken) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(_) => {
                cancel.cancelled().await;
                return;
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
            _ = cancel.cancelled() => {}
        }
    }
    #[cfg(not(unix))]
    {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = cancel.cancelled() => {}
        }
    }
}

/// Read frames from `reader`, push every frame through the policy pipeline,
/// and forward / block / redact accordingly. Block responses are written back
/// to the client via `to_client`; allowed/redacted frames go to `to_child`.
async fn pump_with_intercept<R>(
    mut reader: R,
    to_child: Arc<Mutex<tokio::process::ChildStdin>>,
    to_client: Arc<Mutex<tokio::io::Stdout>>,
    engine: Arc<PolicyEngine>,
    audit: Arc<AuditWriter>,
    session_id: Arc<String>,
    config: Arc<SessionConfig>,
    cancel: CancellationToken,
) -> Result<(), AppError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut buf = BytesMut::with_capacity(8192);
    let mut codec = ContentLengthCodec::default();
    let mut chunk = vec![0u8; 4096];

    loop {
        if cancel.is_cancelled() {
            return Ok(());
        }

        // Try to decode a frame from buffered data first.
        match codec.decode(&mut buf) {
            Ok(Some(frame)) => {
                let outcome = intercept(
                    frame,
                    engine.as_ref(),
                    audit.as_ref(),
                    session_id.as_ref(),
                    &config,
                )
                .await?;
                match outcome {
                    InterceptOutcome::Forward(f) | InterceptOutcome::PassThrough(f) => {
                        let bytes = frame_to_wire_bytes(&f.body);
                        let mut stdin = to_child.lock().await;
                        stdin.write_all(&bytes).await.map_err(|e| {
                            AppError::Upstream(format!("upstream write failed: {e}"))
                        })?;
                        stdin.flush().await.map_err(|e| {
                            AppError::Upstream(format!("upstream flush failed: {e}"))
                        })?;
                    }
                    InterceptOutcome::BlockResponse(f) => {
                        let bytes = frame_to_wire_bytes(&f.body);
                        let mut out = to_client.lock().await;
                        out.write_all(&bytes).await?;
                        out.flush().await?;
                    }
                }
                continue;
            }
            Ok(None) => {} // need more data
            Err(e) => {
                eprintln!("argos: malformed wire data on stdin: {e}");
                return Ok(());
            }
        }

        let n = tokio::select! {
            r = reader.read(&mut chunk) => r?,
            _ = cancel.cancelled() => return Ok(()),
        };
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

/// Forward upstream child output to the client without policy evaluation.
async fn pump_raw<R>(
    mut reader: R,
    to_client: Arc<Mutex<tokio::io::Stdout>>,
    cancel: CancellationToken,
) -> Result<(), AppError>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut chunk = vec![0u8; 4096];
    loop {
        let n = tokio::select! {
            r = reader.read(&mut chunk) => r?,
            _ = cancel.cancelled() => return Ok(()),
        };
        if n == 0 {
            return Ok(());
        }
        let mut out = to_client.lock().await;
        out.write_all(&chunk[..n]).await?;
        out.flush().await?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_one_complete_frame() {
        let mut codec = ContentLengthCodec::default();
        let mut buf = BytesMut::from("Content-Length: 7\r\n\r\n{\"k\":1}".as_bytes());
        let frame = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(&frame.body[..], b"{\"k\":1}");
        assert!(buf.is_empty());
    }

    #[test]
    fn returns_none_on_partial_header() {
        let mut codec = ContentLengthCodec::default();
        let mut buf = BytesMut::from("Content-Length: 7\r\n".as_bytes());
        assert!(codec.decode(&mut buf).unwrap().is_none());
    }

    #[test]
    fn returns_none_on_partial_body() {
        let mut codec = ContentLengthCodec::default();
        let mut buf = BytesMut::from("Content-Length: 10\r\n\r\nabc".as_bytes());
        assert!(codec.decode(&mut buf).unwrap().is_none());
    }

    #[test]
    fn decodes_two_consecutive_frames() {
        let mut codec = ContentLengthCodec::default();
        let mut buf =
            BytesMut::from("Content-Length: 2\r\n\r\nABContent-Length: 2\r\n\r\nCD".as_bytes());
        let f1 = codec.decode(&mut buf).unwrap().unwrap();
        let f2 = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(&f1.body[..], b"AB");
        assert_eq!(&f2.body[..], b"CD");
    }

    #[test]
    fn encode_round_trip() {
        let mut codec = ContentLengthCodec::default();
        let mut buf = BytesMut::new();
        codec.encode(b"{\"x\":1}".as_slice(), &mut buf).unwrap();
        assert_eq!(&buf[..], b"Content-Length: 7\r\n\r\n{\"x\":1}");
    }

    #[test]
    fn rejects_missing_content_length() {
        let mut codec = ContentLengthCodec::default();
        let mut buf = BytesMut::from("Content-Type: x\r\n\r\nbody".as_bytes());
        assert!(codec.decode(&mut buf).is_err());
    }
}
