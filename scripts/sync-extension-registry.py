#!/usr/bin/env python3
"""Keep .specify/extensions/.registry's manifest_hash in sync with the actual
SHA-256 of each .specify/extensions/<id>/extension.yml.

Why: spec-kit only updates manifest_hash during `specify extension add --dev`,
which has a destructive source==dest bug for in-tree extensions. We edit
manifests in place, so the registry would otherwise drift silently.

Two modes:

Pre-commit hook (default — paths passed as args):
  - If no staged extension.yml files: no-op, exit 0.
  - If the recorded hash already matches: no-op, exit 0.
  - If the hash drifted: update .registry and exit 1 with a "git add" prompt.
    Pre-commit standard pattern — user re-stages and re-commits.

Verify mode (--verify):
  - Walks every .specify/extensions/<id>/extension.yml and compares against
    the registry. Read-only — never modifies files.
  - Reports: hash mismatches, extensions on disk missing from the registry,
    and extensions in the registry missing from disk.
  - Exits 0 only if everything is consistent. For use in `just ci` and CI.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

REGISTRY_PATH = Path(".specify/extensions/.registry")
EXTENSIONS_ROOT = Path(".specify/extensions")
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


def _verify_all() -> int:
    if not REGISTRY_PATH.exists():
        print(f"error: {REGISTRY_PATH} missing", file=sys.stderr)
        return 1

    registry = json.loads(REGISTRY_PATH.read_text())
    registered = set(registry.get("extensions", {}).keys())
    on_disk = {p.parent.name for p in EXTENSIONS_ROOT.glob("*/extension.yml")}

    errors: list[str] = []
    has_unregistered = False
    has_missing_on_disk = False
    has_hash_mismatch = False

    for ext_id in sorted(on_disk - registered):
        has_unregistered = True
        errors.append(f"  {ext_id}: extension.yml on disk but not in registry")

    for ext_id in sorted(registered - on_disk):
        has_missing_on_disk = True
        errors.append(f"  {ext_id}: registered but extension.yml missing on disk")

    for ext_id in sorted(registered & on_disk):
        manifest_path = EXTENSIONS_ROOT / ext_id / "extension.yml"
        actual = _compute_hash(manifest_path)
        recorded = registry["extensions"][ext_id].get("manifest_hash", "")
        if actual != recorded:
            has_hash_mismatch = True
            errors.append(
                f"  {ext_id}: hash mismatch "
                f"(recorded {recorded or '(missing)'}, actual {actual})"
            )

    if errors:
        print("Extension registry verification failed:", file=sys.stderr)
        for e in errors:
            print(e, file=sys.stderr)
        print("\nFix:", file=sys.stderr)
        if has_hash_mismatch:
            print(
                "  - Hash mismatch: stage the affected extension.yml files and "
                "commit — the pre-commit hook will rewrite .registry.",
                file=sys.stderr,
            )
        if has_unregistered:
            print(
                "  - On disk but not registered: add via "
                "`specify extension add --dev <id>` or remove the directory. "
                "The pre-commit hook does NOT auto-register new extensions.",
                file=sys.stderr,
            )
        if has_missing_on_disk:
            print(
                "  - Registered but missing on disk: restore the extension.yml "
                "file, or manually remove the entry from "
                ".specify/extensions/.registry.",
                file=sys.stderr,
            )
        return 1

    print(f"OK: {len(registered)} extension(s) in sync with registry.")
    return 0


def main(args: list[str]) -> int:
    if args and args[0] == "--verify":
        if len(args) > 1:
            print("usage: sync-extension-registry.py [--verify | <paths>...]", file=sys.stderr)
            return 2
        return _verify_all()

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
