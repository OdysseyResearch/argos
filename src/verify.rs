//! Audit log verification subcommand (T034/T035).
//!
//! Reads a JSONL audit log line by line, recomputes each entry's `entry_hash`
//! using the canonical "blank then SHA-256" convention, and asserts that the
//! chain is intact: each `prev_hash` equals the previous entry's `entry_hash`,
//! with the first entry rooted at the 64-zero genesis (FR-011, FR-012, SC-004).

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::audit::{writer::compute_entry_hash, AuditEntry};
use crate::error::AppError;

const GENESIS_PREV_HASH: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

/// Verify the SHA-256 hash chain of a JSONL audit log file (SC-004).
///
/// Returns `Ok(())` if the chain verifies cleanly, otherwise [`AppError::Other`]
/// with a human-readable description identifying the entry index where the
/// chain breaks.
pub fn verify_audit_log(path: &Path) -> Result<(), AppError> {
    if !path.is_file() {
        return Err(AppError::Other(anyhow::anyhow!(
            "audit log not found: {}",
            path.display()
        )));
    }

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut prev_entry_hash = GENESIS_PREV_HASH.to_string();
    let mut count: u64 = 0;
    let mut expected_sequence: u64 = 1;

    for (idx, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        if line.is_empty() {
            continue;
        }
        let entry: AuditEntry = serde_json::from_str(&line).map_err(|e| {
            AppError::Other(anyhow::anyhow!(
                "entry {idx} is not valid JSON: {e}"
            ))
        })?;

        if entry.prev_hash != prev_entry_hash {
            return Err(AppError::Other(anyhow::anyhow!(
                "Chain broken at entry {idx}: prev_hash mismatch (expected {}, got {})",
                prev_entry_hash,
                entry.prev_hash
            )));
        }

        let recomputed = compute_entry_hash(&entry).map_err(AppError::Audit)?;
        if recomputed != entry.entry_hash {
            return Err(AppError::Other(anyhow::anyhow!(
                "Chain broken at entry {idx}: entry_hash mismatch (recomputed {}, stored {})",
                recomputed,
                entry.entry_hash
            )));
        }

        if entry.sequence != expected_sequence {
            return Err(AppError::Other(anyhow::anyhow!(
                "Chain broken at entry {idx}: expected sequence {}, got {}",
                expected_sequence,
                entry.sequence
            )));
        }

        prev_entry_hash = entry.entry_hash.clone();
        expected_sequence += 1;
        count += 1;
    }

    println!("Chain intact: {count} entries verified.");
    Ok(())
}
