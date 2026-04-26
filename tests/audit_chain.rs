//! Audit chain integrity tests (FR-011, FR-012, SC-004).

use std::io::{BufRead, BufReader};

use argos::audit::{
    AuditEntry, AuditWriter, DecisionLabel, MessageType,
};
use uuid::Uuid;

fn sample_entry(message_type: MessageType, decision: DecisionLabel) -> AuditEntry {
    AuditEntry {
        timestamp: "2026-04-25T10:00:00.000000Z".to_string(),
        sequence: 0,
        prev_hash: String::new(),
        entry_hash: String::new(),
        session_id: "00000000-0000-0000-0000-000000000000".to_string(),
        message_type,
        decision,
        tool_or_resource: "read_file".to_string(),
        arguments: serde_json::json!({"path": "/workspace/x"}),
        arguments_truncated: false,
        policy_rule_matched: Some("read_file:allow[0]".to_string()),
        reason: None,
        agent: "test-agent".to_string(),
        policy_version: "0.1".to_string(),
        org_id: None,
        tenant_id: None,
        dry_run: None,
    }
}

fn read_jsonl(path: &std::path::Path) -> Vec<AuditEntry> {
    let f = std::fs::File::open(path).unwrap();
    BufReader::new(f)
        .lines()
        .map(|l| l.unwrap())
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<AuditEntry>(&l).unwrap())
        .collect()
}

const GENESIS: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

#[tokio::test]
async fn first_entry_uses_genesis_prev_hash() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let writer = AuditWriter::open(tmp.path(), Uuid::nil(), "test", "0.1").unwrap();
    writer
        .write(sample_entry(MessageType::ToolsCall, DecisionLabel::Allowed))
        .await
        .unwrap();
    writer.flush().await.unwrap();

    let entries = read_jsonl(tmp.path());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].prev_hash, GENESIS);
    assert_eq!(entries[0].sequence, 1);
    assert!(entries[0].entry_hash.starts_with("sha256:"));
    assert_eq!(entries[0].entry_hash.len(), "sha256:".len() + 64);
}

#[tokio::test]
async fn entry_hash_is_reproducible_with_blank_convention() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let writer = AuditWriter::open(tmp.path(), Uuid::nil(), "test", "0.1").unwrap();
    writer
        .write(sample_entry(MessageType::ToolsCall, DecisionLabel::Allowed))
        .await
        .unwrap();
    writer.flush().await.unwrap();

    let entries = read_jsonl(tmp.path());
    let stored_hash = entries[0].entry_hash.clone();
    let recomputed = argos::audit::writer::compute_entry_hash(&entries[0]).unwrap();
    assert_eq!(stored_hash, recomputed);
}

#[tokio::test]
async fn sequential_entries_chain_correctly() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let writer = AuditWriter::open(tmp.path(), Uuid::nil(), "test", "0.1").unwrap();

    for _ in 0..5 {
        writer
            .write(sample_entry(MessageType::ToolsCall, DecisionLabel::Allowed))
            .await
            .unwrap();
    }
    writer.flush().await.unwrap();

    let entries = read_jsonl(tmp.path());
    assert_eq!(entries.len(), 5);

    // Genesis chain bootstrap
    assert_eq!(entries[0].prev_hash, GENESIS);

    // Each entry's prev_hash equals the previous entry's entry_hash.
    for i in 1..entries.len() {
        assert_eq!(entries[i].prev_hash, entries[i - 1].entry_hash);
    }

    // Sequence numbers are monotonic and 1-based.
    for (i, entry) in entries.iter().enumerate() {
        assert_eq!(entry.sequence, (i + 1) as u64);
    }
}

#[tokio::test]
async fn tampering_with_an_entry_breaks_the_chain() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let writer = AuditWriter::open(tmp.path(), Uuid::nil(), "test", "0.1").unwrap();

    for _ in 0..3 {
        writer
            .write(sample_entry(MessageType::ToolsCall, DecisionLabel::Allowed))
            .await
            .unwrap();
    }
    writer.flush().await.unwrap();

    // Mutate entry 1's `tool_or_resource` field after the fact and re-verify.
    let mut entries = read_jsonl(tmp.path());
    entries[1].tool_or_resource = "write_file".to_string();

    let recomputed = argos::audit::writer::compute_entry_hash(&entries[1]).unwrap();
    assert_ne!(
        entries[1].entry_hash, recomputed,
        "tampered entry must produce a different hash"
    );
}
