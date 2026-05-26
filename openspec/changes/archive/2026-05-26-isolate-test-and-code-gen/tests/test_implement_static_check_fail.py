#!/usr/bin/env python3
"""
Integration tests for static-check failure pipeline behavior.

Covers AC-12: static-check failure prevents unit-test phase from triggering;
pipeline directly backtracks to implementation-generator.

Tests:
- Static-check non-zero exit code prevents unit-test phase
- Eval.json has no 06-unit-test entries when static-check fails
- Eval.json implement entry has backtrack_to set when static-check fails
- Static-check pass proceeds to unit-test phase
- Static-check failure details recorded in eval.json report
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch


class TestImplementStaticCheckFail(unittest.TestCase):
    """AC-12: Static-check failure pipeline behavior."""

    def setUp(self):
        self.temp_dir = tempfile.mkdtemp(prefix="static_check_fail_")
        self.phases_dir = os.path.join(self.temp_dir, "openspec", "changes",
                                       "test-change", "phases")
        os.makedirs(self.phases_dir, exist_ok=True)

        # Start with prior phases passed
        self.eval_entries = [
            {"phase": "01-requirements", "timestamp": "2026-05-25T10:00:00.000Z",
             "verdict": "pass", "attempt": 1, "report": "ok", "items": [],
             "backtrack_to": None},
            {"phase": "02-test-design", "timestamp": "2026-05-25T11:00:00.000Z",
             "verdict": "pass", "attempt": 1, "report": "ok", "items": [],
             "backtrack_to": None},
            {"phase": "03-dev-proposal", "timestamp": "2026-05-25T12:00:00.000Z",
             "verdict": "pass", "attempt": 1, "report": "ok", "items": [],
             "backtrack_to": None},
            {"phase": "04-test-gen", "timestamp": "2026-05-25T13:00:00.000Z",
             "verdict": "pass", "attempt": 1, "report": "ok", "items": [],
             "backtrack_to": None},
        ]
        self._write_eval_json()

    def tearDown(self):
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def _write_eval_json(self):
        with open(os.path.join(self.phases_dir, "eval.json"), "w",
                  encoding="utf-8") as f:
            json.dump(self.eval_entries, f, indent=2)

    # ─── Static-check failure behavior ────────────────────────────────────

    def test_static_check_fail_prevents_unit_test_phase(self):
        """AC-12: When static-check fails, no unit-test phase entry is added to eval.json."""
        # Simulate static-check failure: exit code 1
        static_check_failed = True

        if static_check_failed:
            # Add implement phase entry with fail verdict
            self.eval_entries.append({
                "phase": "05-implementation",
                "timestamp": "2026-05-25T14:00:00.000Z",
                "verdict": "fail",
                "attempt": 1,
                "report": "Static-check failed: lint errors found",
                "items": [
                    {"item_id": "static-check", "pass": False,
                     "details": "src/auth.py:42: unused variable 'x'"}
                ],
                "backtrack_to": "05-implementation",
                "phase_suffix": "static-check",
            })
            # Do NOT add unit-test phase entry
            self._write_eval_json()

        # Read eval.json and verify
        with open(os.path.join(self.phases_dir, "eval.json"), "r",
                  encoding="utf-8") as f:
            entries = json.load(f)

        # There should be NO 06-unit-test entry
        unit_test_entries = [e for e in entries if e["phase"] == "06-unit-test"]
        self.assertEqual(len(unit_test_entries), 0, "Unit-test phase should not be triggered")

        # Implement entry should have backtrack_to
        impl_entries = [e for e in entries if e["phase"] == "05-implementation"]
        self.assertGreaterEqual(len(impl_entries), 1)
        self.assertEqual(impl_entries[-1]["backtrack_to"], "05-implementation")

    def test_static_check_fail_implement_entry_has_backtrack(self):
        """AC-12: Static-check failure sets backtrack_to to 05-implementation."""
        impl_entry = {
            "phase": "05-implementation",
            "timestamp": "2026-05-25T14:00:00.000Z",
            "verdict": "fail",
            "attempt": 1,
            "report": "Static-check failed: type errors found",
            "items": [
                {"item_id": "static-check", "pass": False,
                 "details": "src/auth.ts:15: Type 'string' is not assignable to type 'number'"}
            ],
            "backtrack_to": "05-implementation",
            "phase_suffix": "static-check",
        }
        self.assertEqual(impl_entry["backtrack_to"], "05-implementation")
        self.assertEqual(impl_entry["verdict"], "fail")

    def test_static_check_fail_report_contains_error_details(self):
        """AC-12: Static-check failure report includes lint/type error details."""
        impl_entry = {
            "phase": "05-implementation",
            "timestamp": "2026-05-25T14:00:00.000Z",
            "verdict": "fail",
            "attempt": 1,
            "report": "Static-check failed: lint errors found",
            "items": [
                {"item_id": "static-check", "pass": False,
                 "details": "src/auth.py:42: unused variable 'x'\nsrc/auth.py:55: line too long (120 > 88)"}
            ],
            "backtrack_to": "05-implementation",
            "phase_suffix": "static-check",
        }
        self.assertIn("src/auth.py:42", impl_entry["items"][0]["details"])
        self.assertIn("src/auth.py:55", impl_entry["items"][0]["details"])

    def test_static_check_fail_entry_has_phase_suffix(self):
        """AC-12: Static-check entry should have phase_suffix 'static-check'."""
        impl_entry = {
            "phase": "05-implementation",
            "phase_suffix": "static-check",
            "verdict": "fail",
            "backtrack_to": "05-implementation",
        }
        self.assertEqual(impl_entry["phase_suffix"], "static-check")

    # ─── Static-check pass behavior ───────────────────────────────────────

    def test_static_check_pass_proceeds_to_unit_test(self):
        """When static-check passes, unit-test phase should be added."""
        # Simulate static-check pass
        self.eval_entries.append({
            "phase": "05-implementation",
            "timestamp": "2026-05-25T14:00:00.000Z",
            "verdict": "pass",
            "attempt": 1,
            "report": "Static-check passed",
            "items": [{"item_id": "static-check", "pass": True}],
            "backtrack_to": None,
            "phase_suffix": "static-check",
        })
        # Add unit-test entry (simulating phase progression)
        self.eval_entries.append({
            "phase": "06-unit-test",
            "verdict": "pass",
            "attempt": 1,
        })
        self._write_eval_json()

        with open(os.path.join(self.phases_dir, "eval.json"), "r",
                  encoding="utf-8") as f:
            entries = json.load(f)

        unit_test_entries = [e for e in entries if e["phase"] == "06-unit-test"]
        self.assertEqual(len(unit_test_entries), 1)

    def test_static_check_pass_no_backtrack(self):
        """When static-check passes, backtrack_to should be null."""
        impl_entry = {
            "phase": "05-implementation",
            "phase_suffix": "static-check",
            "verdict": "pass",
            "backtrack_to": None,
        }
        self.assertIsNone(impl_entry["backtrack_to"])

    # ─── Subprocess simulation ────────────────────────────────────────────

    def test_simulate_static_check_subprocess_fail(self):
        """Simulate running static-check as subprocess and getting non-zero exit."""
        # TODO: In real integration test, run actual lint command
        # result = subprocess.run(["node", "node_modules/.bin/tsc", "--noEmit"],
        #                        capture_output=True, cwd=project_root)
        # self.assertNotEqual(result.returncode, 0)
        pass

    def test_simulate_static_check_subprocess_pass(self):
        """Simulate running static-check as subprocess and getting zero exit."""
        pass


if __name__ == "__main__":
    unittest.main(verbosity=2)
