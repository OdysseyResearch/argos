"""Tests for scripts/sync-extension-registry.py --verify mode.

Each test creates a temporary .specify/extensions/ tree and runs the script
with cwd set to that tree, so the script's relative paths resolve into the
fixture without import gymnastics on the hyphenated filename.

Run with: just test-scripts
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPT = REPO_ROOT / "scripts" / "sync-extension-registry.py"


def _hash_bytes(content: bytes) -> str:
    return "sha256:" + hashlib.sha256(content).hexdigest()


@pytest.fixture
def fixture_root(tmp_path: Path) -> Path:
    (tmp_path / ".specify" / "extensions").mkdir(parents=True)
    return tmp_path


def _write_extension(root: Path, name: str, body: bytes) -> bytes:
    ext_dir = root / ".specify" / "extensions" / name
    ext_dir.mkdir(parents=True, exist_ok=True)
    (ext_dir / "extension.yml").write_bytes(body)
    return body


def _write_registry(root: Path, entries: dict) -> None:
    registry = {"schema_version": "1.0", "extensions": entries}
    (root / ".specify" / "extensions" / ".registry").write_text(
        json.dumps(registry, indent=2)
    )


def _run_verify(root: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(SCRIPT), "--verify"],
        cwd=root,
        capture_output=True,
        text=True,
    )


def test_all_in_sync_returns_zero(fixture_root: Path) -> None:
    body = _write_extension(fixture_root, "foo", b"name: foo\n")
    _write_registry(fixture_root, {"foo": {"manifest_hash": _hash_bytes(body)}})

    result = _run_verify(fixture_root)

    assert result.returncode == 0, result.stderr
    assert "OK: 1 extension(s)" in result.stdout


def test_hash_mismatch_returns_one_with_mismatch_remediation(
    fixture_root: Path,
) -> None:
    _write_extension(fixture_root, "foo", b"name: foo\n")
    _write_registry(fixture_root, {"foo": {"manifest_hash": "sha256:wrong"}})

    result = _run_verify(fixture_root)

    assert result.returncode == 1
    assert "hash mismatch" in result.stderr
    assert "pre-commit hook will rewrite .registry" in result.stderr


def test_extension_on_disk_not_registered(fixture_root: Path) -> None:
    _write_extension(fixture_root, "ghost", b"name: ghost\n")
    _write_registry(fixture_root, {})

    result = _run_verify(fixture_root)

    assert result.returncode == 1
    assert "ghost: extension.yml on disk but not in registry" in result.stderr
    assert "specify extension add --dev" in result.stderr
    assert "pre-commit hook will rewrite" not in result.stderr


def test_registered_but_missing_on_disk(fixture_root: Path) -> None:
    _write_registry(fixture_root, {"phantom": {"manifest_hash": "sha256:abc"}})

    result = _run_verify(fixture_root)

    assert result.returncode == 1
    assert "phantom: registered but extension.yml missing on disk" in result.stderr
    assert "manually remove the entry" in result.stderr


def test_registry_missing_returns_one(fixture_root: Path) -> None:
    result = _run_verify(fixture_root)

    assert result.returncode == 1
    assert "missing" in result.stderr


def test_multiple_categories_print_only_relevant_remediations(
    fixture_root: Path,
) -> None:
    _write_extension(fixture_root, "ghost", b"name: ghost\n")
    body = _write_extension(fixture_root, "foo", b"name: foo\n")
    _write_registry(fixture_root, {"foo": {"manifest_hash": _hash_bytes(body)}})

    result = _run_verify(fixture_root)

    assert result.returncode == 1
    assert "ghost: extension.yml on disk but not in registry" in result.stderr
    assert "specify extension add --dev" in result.stderr
    assert "hash mismatch" not in result.stderr
    assert "pre-commit hook will rewrite" not in result.stderr
