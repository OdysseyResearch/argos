//! Argos — MCP security proxy.
//!
//! The **stable public library API** lives in [`policy`] (engine, request,
//! decision, errors) and [`audit`] (writer, entry, errors). All other
//! modules are exposed for the `argos-proxy` binary's use only and are
//! `#[doc(hidden)]` — they are not part of the SemVer-stable surface and
//! may change between minor releases.

pub mod audit;
pub mod policy;

#[doc(hidden)]
pub mod cli;
#[doc(hidden)]
pub mod error;
#[doc(hidden)]
pub mod proxy;
#[doc(hidden)]
pub mod transport;
#[doc(hidden)]
pub mod verify;
