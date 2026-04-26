//! Audit log verification subcommand (T034/T035).

use std::path::Path;

use crate::error::AppError;

/// Verify the SHA-256 hash chain of a JSONL audit log file.
///
/// T035 implements the full algorithm: read JSONL, recompute each entry's
/// hash with the canonical `entry_hash=""` convention, assert chain integrity.
pub fn verify_audit_log(_path: &Path) -> Result<(), AppError> {
    // T035 implements this — full chain verification.
    Err(AppError::Other(anyhow::anyhow!(
        "verify subcommand not yet implemented (T035)"
    )))
}
