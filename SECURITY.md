# Security Policy

## Supported Versions

Argos is pre-release software. Security fixes are applied to the latest commit on `main` only.
No backport policy exists until a stable release (1.0.0) is declared.

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Use GitHub's private vulnerability reporting:
**[Report a vulnerability](https://github.com/OdysseyResearch/argos/security/advisories/new)**

Include in your report:

- A description of the vulnerability and its potential impact
- Steps to reproduce or a proof-of-concept (if available)
- The version or commit hash you tested against
- Any suggested mitigations you have identified

You will receive an acknowledgement within 72 hours. If the issue is confirmed, a fix will be
prioritised and you will be kept informed of progress. Credit will be given in the release
notes unless you prefer to remain anonymous.

## Scope

The following are in scope for security reports:

- `argos-proxy` binary — policy bypass, audit log tampering, or data egress bugs
- Policy engine — evaluation logic errors that allow a call that should be blocked
- Audit writer — Merkle chain integrity bugs
- MCP transport adapters — privilege escalation or injection via the proxy

The following are out of scope:

- Vulnerabilities in upstream MCP servers (report those to the respective projects)
- Denial-of-service issues with no security impact beyond availability
- Issues requiring physical access to the machine running the proxy

## Disclosure Policy

Argos follows a coordinated disclosure model. Once a fix is available and released, the
vulnerability will be publicly disclosed in the GitHub release notes. The default embargo
period is 90 days from the date of acknowledgement, or until a fix is released, whichever
comes first.
