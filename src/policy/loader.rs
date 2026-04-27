use std::path::Path;

use crate::policy::engine::{build_matcher, PolicyEngine};
use crate::policy::types::{PolicyAction, PolicyError, PolicyFile};

const SUPPORTED_VERSIONS: &[&str] = &["0.1"];

pub(crate) fn load(path: &Path) -> Result<PolicyEngine, PolicyError> {
    if !path.is_file() {
        return Err(PolicyError::NotFound(path.to_path_buf()));
    }
    let raw = std::fs::read_to_string(path)?;
    let file: PolicyFile = toml::from_str(&raw)?;

    if !SUPPORTED_VERSIONS.contains(&file.meta.version.as_str()) {
        return Err(PolicyError::UnsupportedVersion {
            version: file.meta.version.clone(),
            supported: SUPPORTED_VERSIONS.join(", "),
        });
    }

    let mut matchers = Vec::with_capacity(file.rules.len());
    for (idx, rule) in file.rules.iter().enumerate() {
        match (&rule.tool, &rule.resource) {
            (None, None) => {
                return Err(PolicyError::InvalidRule {
                    index: idx,
                    reason: "rule must specify either `tool` or `resource`".into(),
                });
            }
            (Some(_), Some(_)) => {
                return Err(PolicyError::InvalidRule {
                    index: idx,
                    reason: "rule must specify exactly one of `tool` or `resource`, not both"
                        .into(),
                });
            }
            _ => {}
        }

        if rule.action == PolicyAction::Redact {
            match &rule.redact {
                Some(fields) if !fields.is_empty() => {}
                _ => {
                    return Err(PolicyError::InvalidRule {
                        index: idx,
                        reason: "`redact` action requires non-empty `redact` field list".into(),
                    });
                }
            }
        }

        matchers.push(build_matcher(&rule.tool, &rule.resource)?);
    }

    Ok(PolicyEngine { file, matchers })
}
