//! Policy engine decision-path coverage tests (SC-007).
//!
//! Covers all required decision paths: allow exact, block exact, redact,
//! wildcard `*`, deny-by-default, resource glob, first-match-wins ordering,
//! and `path_prefix` constraints.

use std::io::Write;

use argos::policy::{PolicyDecision, PolicyEngine, PolicyError, PolicyRequest};

fn write_policy(toml: &str) -> tempfile::NamedTempFile {
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    tmp.write_all(toml.as_bytes()).expect("write");
    tmp.flush().expect("flush");
    tmp
}

fn json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap()
}

#[test]
fn allow_exact_tool_match() {
    let policy = write_policy(
        r#"
[meta]
version = "0.1"

[[rules]]
tool = "read_file"
action = "allow"
tags = []
"#,
    );
    let engine = PolicyEngine::load(policy.path()).unwrap();
    let req = PolicyRequest::tool("read_file", json(r#"{"path":"/x"}"#));
    assert!(matches!(engine.evaluate(&req), PolicyDecision::Allow { rule_id: 0 }));
}

#[test]
fn block_exact_tool_match() {
    let policy = write_policy(
        r#"
[meta]
version = "0.1"

[[rules]]
tool = "write_file"
action = "block"
reason = "no writes"
tags = []
"#,
    );
    let engine = PolicyEngine::load(policy.path()).unwrap();
    let req = PolicyRequest::tool("write_file", json("{}"));
    match engine.evaluate(&req) {
        PolicyDecision::Block { reason, rule_id: 0 } => assert_eq!(reason, "no writes"),
        d => panic!("expected Block, got {:?}", d),
    }
}

#[test]
fn redact_with_field_list() {
    let policy = write_policy(
        r#"
[meta]
version = "0.1"

[[rules]]
tool = "read_file"
action = "redact"
redact = ["token", "secret"]
tags = []
"#,
    );
    let engine = PolicyEngine::load(policy.path()).unwrap();
    let req = PolicyRequest::tool("read_file", json("{}"));
    match engine.evaluate(&req) {
        PolicyDecision::Redact { fields, rule_id: 0 } => {
            assert_eq!(fields, vec!["token".to_string(), "secret".to_string()]);
        }
        d => panic!("expected Redact, got {:?}", d),
    }
}

#[test]
fn wildcard_tool_match() {
    let policy = write_policy(
        r#"
[meta]
version = "0.1"

[[rules]]
tool = "*"
action = "block"
reason = "default deny"
tags = []
"#,
    );
    let engine = PolicyEngine::load(policy.path()).unwrap();
    let req = PolicyRequest::tool("anything_goes", json("{}"));
    assert!(matches!(engine.evaluate(&req), PolicyDecision::Block { .. }));
}

#[test]
fn deny_by_default_when_no_rule_matches() {
    let policy = write_policy(
        r#"
[meta]
version = "0.1"

[[rules]]
tool = "read_file"
action = "allow"
tags = []
"#,
    );
    let engine = PolicyEngine::load(policy.path()).unwrap();
    let req = PolicyRequest::tool("write_file", json("{}"));
    assert!(matches!(engine.evaluate(&req), PolicyDecision::DenyByDefault));
}

#[test]
fn resource_glob_match() {
    let policy = write_policy(
        r#"
[meta]
version = "0.1"

[[rules]]
resource = "file:///workspace/src/**"
action = "allow"
tags = []
"#,
    );
    let engine = PolicyEngine::load(policy.path()).unwrap();
    let req_inside = PolicyRequest::resource("file:///workspace/src/main.rs");
    let req_outside = PolicyRequest::resource("file:///etc/passwd");
    assert!(matches!(engine.evaluate(&req_inside), PolicyDecision::Allow { .. }));
    assert!(matches!(engine.evaluate(&req_outside), PolicyDecision::DenyByDefault));
}

#[test]
fn first_match_wins_order() {
    let policy = write_policy(
        r#"
[meta]
version = "0.1"

[[rules]]
tool = "read_file"
action = "allow"
tags = []

[[rules]]
tool = "*"
action = "block"
reason = "catch-all"
tags = []
"#,
    );
    let engine = PolicyEngine::load(policy.path()).unwrap();
    let req = PolicyRequest::tool("read_file", json("{}"));
    assert!(matches!(engine.evaluate(&req), PolicyDecision::Allow { rule_id: 0 }));
    let req_other = PolicyRequest::tool("write_file", json("{}"));
    assert!(matches!(engine.evaluate(&req_other), PolicyDecision::Block { rule_id: 1, .. }));
}

#[test]
fn path_prefix_constraint_pass_and_fail() {
    let policy = write_policy(
        r#"
[meta]
version = "0.1"

[[rules]]
tool = "read_file"
action = "allow"
constraints = { path_prefix = "/workspace" }
tags = []
"#,
    );
    let engine = PolicyEngine::load(policy.path()).unwrap();
    let inside = PolicyRequest::tool("read_file", json(r#"{"path":"/workspace/main.rs"}"#));
    let outside = PolicyRequest::tool("read_file", json(r#"{"path":"/etc/passwd"}"#));
    assert!(matches!(engine.evaluate(&inside), PolicyDecision::Allow { .. }));
    assert!(matches!(engine.evaluate(&outside), PolicyDecision::DenyByDefault));
}

#[test]
fn unsupported_version_is_hard_error() {
    let policy = write_policy(
        r#"
[meta]
version = "9.9"
"#,
    );
    match PolicyEngine::load(policy.path()) {
        Err(PolicyError::UnsupportedVersion { .. }) => {}
        other => panic!("expected UnsupportedVersion, got {:?}", other),
    }
}

#[test]
fn redact_without_fields_is_hard_error() {
    let policy = write_policy(
        r#"
[meta]
version = "0.1"

[[rules]]
tool = "read_file"
action = "redact"
tags = []
"#,
    );
    match PolicyEngine::load(policy.path()) {
        Err(PolicyError::InvalidRule { index: 0, .. }) => {}
        other => panic!("expected InvalidRule, got {:?}", other),
    }
}

#[test]
fn rule_with_both_tool_and_resource_is_hard_error() {
    let policy = write_policy(
        r#"
[meta]
version = "0.1"

[[rules]]
tool = "read_file"
resource = "file:///x"
action = "allow"
tags = []
"#,
    );
    match PolicyEngine::load(policy.path()) {
        Err(PolicyError::InvalidRule { .. }) => {}
        other => panic!("expected InvalidRule, got {:?}", other),
    }
}
