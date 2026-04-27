use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "argos-proxy",
    version,
    about = "MCP security proxy — capability policy enforcement with tamper-evident audit log"
)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Path to the TOML policy file (required for proxy mode).
    #[arg(long)]
    pub policy: Option<PathBuf>,

    /// Path to the JSONL audit log file (required for proxy mode).
    #[arg(long)]
    pub audit_log: Option<PathBuf>,

    /// Operator-supplied agent label written to every audit entry.
    #[arg(long, default_value = "unknown")]
    pub agent: String,

    /// Maximum argument byte size before truncation in audit entries.
    #[arg(long, default_value_t = 65536)]
    pub max_arg_bytes: usize,

    /// Dry-run mode: violations are logged but calls are not blocked.
    #[arg(long)]
    pub dry_run: bool,

    /// Per-request stderr trace logging.
    #[arg(long)]
    pub verbose: bool,

    /// Upstream MCP server URL — activates HTTP/SSE mode.
    #[arg(long)]
    pub upstream: Option<String>,

    /// HTTP listen address (HTTP mode only).
    #[arg(long, default_value = "127.0.0.1")]
    pub bind: String,

    /// HTTP listen port (HTTP mode only).
    #[arg(long, default_value_t = 8080)]
    pub port: u16,

    /// TLS certificate (HTTP mode; reserved for v1.0 mTLS).
    #[arg(long)]
    pub tls_cert: Option<PathBuf>,

    /// TLS key (HTTP mode; reserved for v1.0 mTLS).
    #[arg(long)]
    pub tls_key: Option<PathBuf>,

    /// stdio mode: server command and args after `--`.
    #[arg(last = true)]
    pub server_command: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Verify the integrity of an audit log's hash chain.
    Verify {
        /// Path to the JSONL audit log to verify.
        #[arg(long)]
        audit_log: PathBuf,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum TransportMode {
    Stdio,
    Http,
}

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("--policy is required")]
    PolicyMissing,
    #[error("--audit-log is required")]
    AuditLogMissing,
    #[error("must specify either --upstream <url> or -- <server-cmd>")]
    NoTransportMode,
    #[error("cannot specify both --upstream and -- <server-cmd>")]
    AmbiguousTransportMode,
    #[error("policy file is not readable: {0}")]
    PolicyUnreadable(PathBuf),
    #[error("audit log path is not writable: {0}")]
    AuditLogUnwritable(PathBuf),
    #[error("--tls-cert and --tls-key must be provided together")]
    TlsConfigIncomplete,
    #[error("TLS certificate not readable: {0}")]
    TlsCertUnreadable(PathBuf),
    #[error("TLS key not readable: {0}")]
    TlsKeyUnreadable(PathBuf),
}

impl CliArgs {
    /// Validate startup invariants and infer transport mode.
    ///
    /// Performs no I/O beyond filesystem readability/writability probes.
    pub fn validate(&self) -> Result<TransportMode, StartupError> {
        let policy = self.policy.as_ref().ok_or(StartupError::PolicyMissing)?;
        let audit_log = self
            .audit_log
            .as_ref()
            .ok_or(StartupError::AuditLogMissing)?;

        let has_upstream = self.upstream.is_some();
        let has_command = !self.server_command.is_empty();
        let mode = match (has_upstream, has_command) {
            (false, false) => return Err(StartupError::NoTransportMode),
            (true, true) => return Err(StartupError::AmbiguousTransportMode),
            (true, false) => TransportMode::Http,
            (false, true) => TransportMode::Stdio,
        };

        if !policy.is_file() {
            return Err(StartupError::PolicyUnreadable(policy.clone()));
        }
        if std::fs::metadata(policy)
            .ok()
            .filter(|m| m.permissions().readonly() || !m.permissions().readonly())
            .is_none()
        {
            return Err(StartupError::PolicyUnreadable(policy.clone()));
        }

        if let Some(parent) = audit_log.parent() {
            if !parent.as_os_str().is_empty() && !parent.is_dir() {
                return Err(StartupError::AuditLogUnwritable(audit_log.clone()));
            }
        }
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(audit_log)
        {
            Ok(_) => {}
            Err(_) => return Err(StartupError::AuditLogUnwritable(audit_log.clone())),
        }

        match (&self.tls_cert, &self.tls_key) {
            (None, None) => {}
            (Some(_), None) | (None, Some(_)) => return Err(StartupError::TlsConfigIncomplete),
            (Some(cert), Some(key)) => {
                if !cert.is_file() {
                    return Err(StartupError::TlsCertUnreadable(cert.clone()));
                }
                if !key.is_file() {
                    return Err(StartupError::TlsKeyUnreadable(key.clone()));
                }
            }
        }

        Ok(mode)
    }
}
