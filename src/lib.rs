//! Argos — MCP security proxy library.
//!
//! Public API surfaces: [`policy`] (engine, request, decision, errors) and
//! [`audit`] (writer, entry, errors). Internal types (`McpRequest`, `McpFrame`,
//! `ProxySession`) are deliberately not re-exported.

pub mod audit;
pub mod policy;

pub(crate) mod cli;
pub(crate) mod error;
pub(crate) mod proxy;
pub(crate) mod transport;
