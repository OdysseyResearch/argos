use std::path::Path;

use globset::{Glob, GlobMatcher};

use crate::policy::types::{PolicyAction, PolicyDecision, PolicyError, PolicyFile, PolicyRequest};

/// Per-rule compiled matcher, parallel to [`PolicyFile::rules`].
#[derive(Debug)]
pub(crate) enum RuleMatcher {
    /// Tool rule — matches by exact name or the `*` catch-all wildcard.
    Tool { name: String },
    /// Resource rule — matches by compiled glob.
    Resource { matcher: GlobMatcher },
}

/// In-memory representation of a loaded policy, ready for evaluation.
#[derive(Debug)]
pub struct PolicyEngine {
    pub(crate) file: PolicyFile,
    pub(crate) matchers: Vec<RuleMatcher>,
}

impl PolicyEngine {
    /// Load and validate a TOML policy file from disk.
    pub fn load(path: &Path) -> Result<Self, PolicyError> {
        crate::policy::loader::load(path)
    }

    /// Evaluate a request against the loaded policy.
    ///
    /// Stateless and `&self` — safe to call from multiple concurrent tasks
    /// without external locking. Returns the first matching decision in
    /// top-to-bottom rule order, or [`PolicyDecision::DenyByDefault`] if no
    /// rule matches.
    pub fn evaluate(&self, request: &PolicyRequest) -> PolicyDecision {
        for (idx, (rule, matcher)) in self.file.rules.iter().zip(self.matchers.iter()).enumerate()
        {
            let is_match = match (request, matcher) {
                (PolicyRequest::Tool { name, arguments }, RuleMatcher::Tool { name: rule_name }) => {
                    if rule_name == "*" || rule_name == name {
                        constraint_matches(rule.constraints.as_ref(), arguments)
                    } else {
                        false
                    }
                }
                (PolicyRequest::Resource { uri }, RuleMatcher::Resource { matcher }) => {
                    matcher.is_match(uri)
                }
                _ => false,
            };

            if !is_match {
                continue;
            }

            return match rule.action {
                PolicyAction::Allow => PolicyDecision::Allow { rule_id: idx },
                PolicyAction::Block => PolicyDecision::Block {
                    reason: rule
                        .reason
                        .clone()
                        .unwrap_or_else(|| "Blocked by policy".to_string()),
                    rule_id: idx,
                },
                PolicyAction::Redact => PolicyDecision::Redact {
                    fields: rule.redact.clone().unwrap_or_default(),
                    rule_id: idx,
                },
            };
        }

        PolicyDecision::DenyByDefault
    }

    /// Returns `meta.version` from the loaded policy, for audit logging.
    pub fn version(&self) -> &str {
        &self.file.meta.version
    }
}

/// Apply tool-rule constraints to a request's arguments.
///
/// In v0.1 only `path_prefix` is supported (FR-008): if present, the matching
/// argument must be a string starting with the configured prefix. Constraint
/// keys other than `path_prefix` are silently ignored — extension is M2 scope.
fn constraint_matches(
    constraints: Option<&std::collections::HashMap<String, String>>,
    arguments: &serde_json::Value,
) -> bool {
    let Some(constraints) = constraints else {
        return true;
    };
    if constraints.is_empty() {
        return true;
    }
    let Some(prefix) = constraints.get("path_prefix") else {
        return true;
    };

    arguments
        .as_object()
        .and_then(|obj| obj.get("path"))
        .and_then(|v| v.as_str())
        .map(|s| s.starts_with(prefix))
        .unwrap_or(false)
}

/// Build a [`RuleMatcher`] from a structurally-validated rule. Used during
/// loader compilation; assumes the rule has already passed the load-time
/// structural validation in [`crate::policy::loader::load`].
pub(crate) fn build_matcher(
    tool: &Option<String>,
    resource: &Option<String>,
) -> Result<RuleMatcher, PolicyError> {
    if let Some(name) = tool {
        return Ok(RuleMatcher::Tool { name: name.clone() });
    }
    if let Some(pattern) = resource {
        let glob = if pattern == "*" {
            Glob::new("**")?
        } else {
            Glob::new(pattern)?
        };
        return Ok(RuleMatcher::Resource {
            matcher: glob.compile_matcher(),
        });
    }
    unreachable!("loader validation guarantees exactly one of tool/resource")
}
