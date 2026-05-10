#!/usr/bin/env python3
"""Pre-commit hook: keep .specify/extensions/.registry's manifest_hash in sync
with the actual SHA-256 of each .specify/extensions/<id>/extension.yml.

Why: spec-kit only updates manifest_hash during `specify extension add --dev`,
which has a destructive source==dest bug for in-tree extensions. We edit
manifests in place, so the registry would otherwise drift silently. This hook
recomputes the hash on every commit that touches an extension manifest and
writes it back to .registry.

Behavior:
  - If no staged extension.yml files: no-op, exit 0.
  - If the recorded hash already matches: no-op, exit 0.
  - If the hash drifted: update .registry and exit 1 with a "git add" prompt.
    Pre-commit standard pattern — user re-stages and re-commits.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

REGISTRY_PATH = Path(".specify/extensions/.registry")
EXTENSIONS_PREFIX = (".specify", "extensions")


def _compute_hash(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def _is_extension_manifest(path: Path) -> bool:
    parts = path.parts
    return (
        len(parts) == 4
        and parts[0:2] == EXTENSIONS_PREFIX
        and parts[3] == "extension.yml"
    )


def main(args: list[str]) -> int:
    manifests = [Path(p) for p in args if _is_extension_manifest(Path(p))]
    if not manifests:
        return 0

    if not REGISTRY_PATH.exists():
        print(
            f"warning: {REGISTRY_PATH} missing — skipping registry sync "
            "(run `specify extension add --dev` first)",
            file=sys.stderr,
        )
        return 0

    registry = json.loads(REGISTRY_PATH.read_text())
    updates: list[tuple[str, str, str]] = []

    for manifest_path in manifests:
        ext_id = manifest_path.parts[2]
        ext_entry = registry.get("extensions", {}).get(ext_id)
        if ext_entry is None:
            print(
                f"warning: extension '{ext_id}' has a manifest at {manifest_path} "
                f"but is not in {REGISTRY_PATH} — skipping",
                file=sys.stderr,
            )
            continue

        actual = _compute_hash(manifest_path)
        recorded = ext_entry.get("manifest_hash", "")
        if actual != recorded:
            ext_entry["manifest_hash"] = actual
            updates.append((ext_id, recorded, actual))

    if not updates:
        return 0

    REGISTRY_PATH.write_text(json.dumps(registry, indent=2))

    print(f"Updated {REGISTRY_PATH}:")
    for ext_id, old, new in updates:
        print(f"  {ext_id}: {old or '(missing)'} → {new}")
    print(f"\nStage the registry change and re-commit:")
    print(f"  git add {REGISTRY_PATH}")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
