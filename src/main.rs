use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;

use argos::audit::AuditWriter;
use argos::cli::{CliArgs, Command, TransportMode};
use argos::error::AppError;
use argos::policy::PolicyEngine;
use argos::transport::stdio::run_stdio_proxy;
use argos::transport::SessionConfig;

#[tokio::main]
async fn main() -> ExitCode {
    let args = CliArgs::parse();

    if let Some(Command::Verify { audit_log }) = &args.command {
        return match argos::verify::verify_audit_log(audit_log) {
            Ok(()) => ExitCode::from(0),
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(1)
            }
        };
    }

    match run_proxy(args).await {
        Ok(()) => ExitCode::from(0),
        Err(e) => {
            eprintln!("argos: {e}");
            ExitCode::from(e.exit_code() as u8)
        }
    }
}

async fn run_proxy(args: CliArgs) -> Result<(), AppError> {
    let mode = args.validate()?;

    let policy_path = args.policy.as_ref().expect("validated");
    let audit_path = args.audit_log.as_ref().expect("validated");

    let engine = PolicyEngine::load(policy_path)?;
    let policy_version = engine.version().to_string();
    let session_id = uuid::Uuid::new_v4();
    let writer = AuditWriter::open(audit_path, session_id, &args.agent, &policy_version)?;

    eprintln!(
        "argos-proxy: policy={} audit={} agent={} mode={:?} session={}",
        policy_path.display(),
        audit_path.display(),
        args.agent,
        mode,
        session_id
    );
    if args.dry_run {
        eprintln!("argos-proxy: WARNING: DRY RUN ACTIVE — policy violations will not be enforced");
    }

    let config = Arc::new(SessionConfig {
        dry_run: args.dry_run,
        max_arg_bytes: args.max_arg_bytes,
        agent: args.agent.clone(),
    });

    let engine = Arc::new(engine);
    let writer = Arc::new(writer);

    match mode {
        TransportMode::Stdio => {
            run_stdio_proxy(
                &args.server_command,
                engine,
                writer.clone(),
                session_id.to_string(),
                config,
            )
            .await?;
        }
        TransportMode::Http => {
            argos::transport::http::run_http_proxy(args, engine, writer.clone(), session_id, config)
                .await?;
        }
    }

    writer.flush().await?;
    Ok(())
}
