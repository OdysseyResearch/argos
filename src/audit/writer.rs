use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::audit::types::{AuditEntry, AuditError};

/// Genesis `prev_hash` used for the first entry in a new log file (FR-012).
pub const GENESIS_PREV_HASH: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

pub(crate) struct AuditWriterInner {
    file: BufWriter<std::fs::File>,
    pub(crate) sequence: u64,
    pub(crate) prev_hash: String,
}

/// Append-only Merkle-chained JSONL audit log writer.
///
/// Cloneable / `Send` / `Sync` via internal `Arc<Mutex<...>>`. Multiple
/// concurrent tasks may call [`Self::write`] — the mutex is held only for the
/// duration of the chained write to keep `prev_hash` ordering deterministic
/// (FR-028).
#[derive(Clone)]
pub struct AuditWriter {
    pub(crate) inner: Arc<Mutex<AuditWriterInner>>,
    pub(crate) session_id: String,
    pub(crate) agent: String,
    pub(crate) policy_version: String,
    pub(crate) path: PathBuf,
}

impl AuditWriter {
    /// Open (or create) the audit log file at `path` in append mode (FR-013).
    pub fn open(
        path: &Path,
        session_id: Uuid,
        agent: &str,
        policy_version: &str,
    ) -> Result<Self, AuditError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|_| AuditError::NotWritable(path.to_path_buf()))?;

        Ok(Self {
            inner: Arc::new(Mutex::new(AuditWriterInner {
                file: BufWriter::new(file),
                sequence: 0,
                prev_hash: GENESIS_PREV_HASH.to_string(),
            })),
            session_id: session_id.to_string(),
            agent: agent.to_string(),
            policy_version: policy_version.to_string(),
            path: path.to_path_buf(),
        })
    }

    /// Write a single audit entry as a JSONL line, computing the entry's hash
    /// and chaining it to the previous entry's `entry_hash`.
    ///
    /// Mutex is acquired for the duration of the hash + write step only.
    pub async fn write(&self, mut entry: AuditEntry) -> Result<(), AuditError> {
        let mut guard = self.inner.lock().await;

        guard.sequence = guard.sequence.saturating_add(1);
        entry.sequence = guard.sequence;
        entry.prev_hash = guard.prev_hash.clone();
        entry.entry_hash = String::new();

        let canonical = serde_json::to_string(&entry)?;
        let digest = Sha256::digest(canonical.as_bytes());
        let entry_hash = format!("sha256:{:x}", digest);

        entry.entry_hash = entry_hash.clone();

        let line = serde_json::to_string(&entry)?;
        guard.file.write_all(line.as_bytes())?;
        guard.file.write_all(b"\n")?;

        guard.prev_hash = entry_hash;
        Ok(())
    }

    /// Flush any buffered audit data to disk (FR-018c).
    pub async fn flush(&self) -> Result<(), AuditError> {
        let mut guard = self.inner.lock().await;
        guard.file.flush()?;
        Ok(())
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn agent(&self) -> &str {
        &self.agent
    }

    pub fn policy_version(&self) -> &str {
        &self.policy_version
    }

    /// Path the writer was opened at — useful for error messages and tests.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Compute the canonical `entry_hash` for an entry per `contracts/audit-log.md`.
///
/// 1. Set `entry_hash` to the empty string.
/// 2. Serialise to compact JSON.
/// 3. SHA-256 over the UTF-8 bytes.
/// 4. Format as `"sha256:<lowercase hex>"`.
///
/// Used by the verifier (T035) to reproduce hashes during chain validation.
pub fn compute_entry_hash(entry: &AuditEntry) -> Result<String, AuditError> {
    let mut clone = entry.clone();
    clone.entry_hash = String::new();
    let canonical = serde_json::to_string(&clone)?;
    let digest = Sha256::digest(canonical.as_bytes());
    Ok(format!("sha256:{:x}", digest))
}
