use thiserror::Error;

use crate::audit::AuditError;
use crate::cli::StartupError;
use crate::policy::PolicyError;

/// Top-level error type for the binary.
///
/// Maps to the documented exit codes:
///  * `1` — startup or policy error ([`AppError::Startup`], [`AppError::Policy`])
///  * `2` — runtime audit-write failure ([`AppError::Audit`])
///  * `3` — upstream subprocess failure ([`AppError::Upstream`])
#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Startup(#[from] StartupError),

    #[error(transparent)]
    Policy(#[from] PolicyError),

    #[error(transparent)]
    Audit(#[from] AuditError),

    #[error("upstream subprocess failed: {0}")]
    Upstream(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl AppError {
    /// Map this error to the documented exit code.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Startup(_) | Self::Policy(_) => 1,
            Self::Audit(_) => 2,
            Self::Upstream(_) => 3,
            Self::Io(_) | Self::Other(_) => 1,
        }
    }
}
