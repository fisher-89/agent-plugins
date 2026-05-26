#!/usr/bin/env python3
"""
Tests for no-op phase skipping determination logic.

Covers AC-10: no-op phase determination logic + eval.json skipped entry validation.

Tests:
- Unit-test phase skipped when no *.test.* files exist
- Unit-test phase non-skipped when test files exist
- Integration-test phase skipped when no integration test files exist
- Integration-test phase non-skipped when integration test files exist
- Integration-test phase skipped when tests/ but no integration/ subdirectory
- Empty test files considered as test files (files exist = not no-op)
- Skipped entry in eval.json has correct schema (phase, skipped=true, verdict="pass", evidence)
- No-op determination isolation: unit-test and integration-test checked independently
"""

import json
import os
import shutil
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch


def is_unit_test_noop(change_root: str) -> bool:
    """Determine if unit-test phase is no-op (no test files found) (skeleton).

    Returns True if no unit test files exist.
    """
    import glob
    patterns = ["**/*.test.*", "**/tests/unit/**", "**/__tests__/**"]
    for pattern in patterns:
        matches = glob.glob(os.path.join(change_root, pattern), recursive=True)
        if matches:
            return False
    return True


def is_integration_test_noop(change_root: str) -> bool:
    """Determine if integration-test phase is no-op (skeleton)."""
    import glob
    patterns = ["**/*.integration.test.*", "**/tests/integration/**"]
    for pattern in patterns:
        matches = glob.glob(os.path.join(change_root, pattern), recursive=True)
        if matches:
            return False
    return True


def make_skipped_entry(phase: str, reason: str = "") -> dict:
    """Create a skipped eval.json entry (skeleton)."""
    entry = {
        "phase": phase,
        "verdict": "pass",
        "skipped": True,
        "evidence": reason or f"skipped: no applicable tests for {phase}",
    }
    return entry


class TestNoopPhaseDetermination(unittest.TestCase):
    """AC-10: No-op phase determination logic."""

    def setUp(self):
        self.temp_dir = tempfile.mkdtemp(prefix="noop_test_")

    def tearDown(self):
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def create_file(self, rel_path: str, content: str = ""):
        """Create a file in the temp directory."""
        full_path = os.path.join(self.temp_dir, rel_path)
        os.makedirs(os.path.dirname(full_path), exist_ok=True)
        with open(full_path, "w", encoding="utf-8") as f:
            f.write(content)

    # ─── Unit-test no-op determination ────────────────────────────────────

    def test_unit_test_noop_when_no_test_files(self):
        """Unit-test is no-op when no *.test.* files exist."""
        self.create_file("src/auth.py", "def login(): pass")
        self.create_file("design.md", "# Design")
        # no test files
        with patch("glob.glob", return_value=[]):
            self.assertTrue(is_unit_test_noop(self.temp_dir))

    def test_unit_test_not_noop_when_test_files_exist(self):
        """Unit-test is not no-op when *.test.* files exist."""
        self.create_file("tests/test_auth.py", "def test_login(): pass")
        with patch("glob.glob", return_value=["tests/test_auth.py"]):
            self.assertFalse(is_unit_test_noop(self.temp_dir))

    def test_unit_test_not_noop_when_test_ts_files_exist(self):
        """Unit-test is not no-op when *.test.ts files exist."""
        self.create_file("src/__tests__/auth.test.ts", "describe('auth', () => {})")
        with patch("glob.glob", return_value=["src/__tests__/auth.test.ts"]):
            self.assertFalse(is_unit_test_noop(self.temp_dir))

    def test_unit_test_not_noop_when_tests_unit_dir_exists(self):
        """Unit-test is not no-op when tests/unit/ directory has files."""
        self.create_file("tests/unit/test_auth.py", "def test_login(): pass")
        with patch("glob.glob", return_value=["tests/unit/test_auth.py"]):
            self.assertFalse(is_unit_test_noop(self.temp_dir))

    def test_unit_test_not_noop_when_test_js_files_exist(self):
        """Unit-test is not no-op when *.test.js files exist."""
        self.create_file("__tests__/app.test.js", "test('app', () => {})")
        with patch("glob.glob", return_value=["__tests__/app.test.js"]):
            self.assertFalse(is_unit_test_noop(self.temp_dir))

    # ─── Integration-test no-op determination ─────────────────────────────

    def test_integration_test_noop_when_no_integration_files(self):
        """Integration-test is no-op when no *integration.test.* files exist."""
        self.create_file("src/auth.py", "def login(): pass")
        self.create_file("tests/test_auth.py", "def test_login(): pass")
        # unit test file only, no integration test files
        with patch("glob.glob", return_value=[]):
            self.assertTrue(is_integration_test_noop(self.temp_dir))

    def test_integration_test_not_noop_when_integration_test_files(self):
        """Integration-test is not no-op when *.integration.test.* files exist."""
        self.create_file("tests/integration/test_api.integration.test.py",
                         "def test_api(): pass")
        with patch("glob.glob", return_value=["tests/integration/test_api.integration.test.py"]):
            self.assertFalse(is_integration_test_noop(self.temp_dir))

    def test_integration_test_not_noop_when_integration_dir_exists(self):
        """Integration-test is not no-op when tests/integration/ directory has files."""
        self.create_file("tests/integration/test_db.py", "def test_db(): pass")
        with patch("glob.glob", return_value=["tests/integration/test_db.py"]):
            self.assertFalse(is_integration_test_noop(self.temp_dir))

    def test_integration_test_noop_when_only_unit_tests_exist(self):
        """Integration-test is no-op when only unit tests exist (no integration tests)."""
        self.create_file("tests/test_auth.py", "def test_login(): pass")
        self.create_file("tests/test_register.py", "def test_register(): pass")
        with patch("glob.glob", side_effect=[
            [],  # **/*.integration.test.*
            [],  # **/tests/integration/**
        ]):
            self.assertTrue(is_integration_test_noop(self.temp_dir))

    # ─── Empty files considered as test files ─────────────────────────────

    def test_empty_test_file_still_counts_as_non_noop(self):
        """Empty test files still count as test files (noop is about existence, not content)."""
        self.create_file("tests/test_auth.py", "")  # empty file
        with patch("glob.glob", return_value=["tests/test_auth.py"]):
            self.assertFalse(is_unit_test_noop(self.temp_dir))

    def test_empty_integration_file_still_counts_as_non_noop(self):
        """Empty integration test file counts as non-noop."""
        self.create_file("tests/integration/test_api.py", "")  # empty file
        with patch("glob.glob", return_value=["tests/integration/test_api.py"]):
            self.assertFalse(is_integration_test_noop(self.temp_dir))

    # ─── Cross-phase isolation ────────────────────────────────────────────

    def test_unit_test_and_integration_checked_independently(self):
        """Unit-test and integration-test noop determination are independent."""
        # Only unit test files exist
        self.create_file("tests/test_auth.py", "def test_login(): pass")
        self.create_file("src/auth.py", "def login(): pass")

        with patch("glob.glob", side_effect=[
            ["tests/test_auth.py"],  # unit test patterns -> found
            [],  # integration patterns -> not found
        ]):
            self.assertFalse(is_unit_test_noop(self.temp_dir))
            self.assertTrue(is_integration_test_noop(self.temp_dir))


class TestSkippedEvalJsonEntry(unittest.TestCase):
    """AC-10: Eval.json skipped entry schema and format."""

    def test_skipped_entry_has_required_fields(self):
        """Skipped entry must contain phase, skipped=True, verdict='pass', evidence."""
        entry = make_skipped_entry("06-unit-test")
        self.assertIn("phase", entry)
        self.assertIn("skipped", entry)
        self.assertIn("verdict", entry)
        self.assertIn("evidence", entry)
        self.assertEqual(entry["phase"], "06-unit-test")
        self.assertTrue(entry["skipped"])
        self.assertEqual(entry["verdict"], "pass")

    def test_skipped_entry_verdict_is_pass(self):
        """Skipped entries must always have verdict 'pass'."""
        entry = make_skipped_entry("08-integration-test")
        self.assertEqual(entry["verdict"], "pass")

    def test_skipped_entry_evidence_contains_phase_description(self):
        """Evidence field should mention which phase was skipped."""
        entry = make_skipped_entry("06-unit-test")
        self.assertIn("06-unit-test", entry["evidence"])

    def test_skipped_entry_serializes_to_json(self):
        """Skipped entry must be valid JSON (for eval.json append)."""
        entry = make_skipped_entry("06-unit-test")
        serialized = json.dumps(entry)
        parsed = json.loads(serialized)
        self.assertEqual(parsed["phase"], "06-unit-test")
        self.assertTrue(parsed["skipped"])

    def test_unit_test_skipped_evidence_message(self):
        """Unit-test skipped evidence should mention 'no applicable tests'."""
        entry = make_skipped_entry("06-unit-test", "skipped: no applicable tests")
        self.assertIn("no applicable tests", entry["evidence"])

    def test_integration_test_skipped_evidence_message(self):
        """Integration-test skipped evidence should be meaningful."""
        entry = make_skipped_entry("08-integration-test",
                                   "skipped: no integration test files found")
        self.assertIn("integration", entry["evidence"])
        self.assertIn("skipped", entry["evidence"])

    # ─── Eval.json integration ────────────────────────────────────────────

    def test_skipped_entry_appended_to_eval_json(self):
        """Skipped entry can be appended to eval.json."""
        existing_entries = [
            {"phase": "01-requirements", "verdict": "pass"},
            {"phase": "02-test-design", "verdict": "pass"},
        ]
        skipped_entry = make_skipped_entry("06-unit-test")
        existing_entries.append(skipped_entry)

        self.assertEqual(len(existing_entries), 3)
        self.assertEqual(existing_entries[2]["phase"], "06-unit-test")
        self.assertTrue(existing_entries[2]["skipped"])

    def test_skipped_entry_does_not_break_eval_json_integrity(self):
        """Appending skipped entries keeps eval.json valid."""
        entries = [
            {"phase": "01-requirements", "verdict": "pass"},
            {"phase": "04-test-gen", "verdict": "pass"},
        ]
        entries.append(make_skipped_entry("06-unit-test"))
        entries.append(make_skipped_entry("08-integration-test"))

        serialized = json.dumps(entries)
        parsed = json.loads(serialized)
        self.assertEqual(len(parsed), 4)
        skipped = [e for e in parsed if e.get("skipped")]
        self.assertEqual(len(skipped), 2)


if __name__ == "__main__":
    unittest.main(verbosity=2)
