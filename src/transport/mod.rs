use bytes::Bytes;

pub mod http;
pub mod stdio;

/// One Content-Length-framed JSON-RPC message.
#[derive(Debug, Clone)]
pub(crate) struct McpFrame {
    pub body: Bytes,
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub dry_run: bool,
    pub max_arg_bytes: usize,
    #[allow(dead_code)]
    pub agent: String,
}
