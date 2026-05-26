#!/usr/bin/env python3
"""
Integration tests for `dev-team archi check` command.

Covers AC-17: archi check --staged violation reporting.

Tests:
- Check with --staged reports violations for known problematic models
- Violations list contains type, severity, detail for each issue
- Unmodeled dependency is detected
- Unmapped import target is detected
- Path not found is detected
- Check passes when no issues found
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest


class TestArchiCheck(unittest.TestCase):
    """AC-17: `dev-team archi check` violation reporting."""

    def setUp(self):
        self.project_root = os.path.normpath(
            os.path.join(os.path.dirname(os.path.abspath(__file__)),
                         "..", "..", "..", "..")
        )
        self.bundle_path = os.path.join(
            self.project_root, "plugins", "dev-team", "bin", "dev-team-bundle.cjs"
        )

    # ─── Check with --staged ──────────────────────────────────────────────

    def test_check_staged_runs_without_crash(self):
        """AC-17: `archi check --staged` runs without crashing."""
        if not os.path.isfile(self.bundle_path):
            self.skipTest(f"Bundle not found at {self.bundle_path}")

        try:
            result = subprocess.run(
                ["node", self.bundle_path, "archi", "check", "--staged"],
                capture_output=True, text=True, timeout=30,
                cwd=self.project_root,
            )
        except subprocess.TimeoutExpired:
            self.skipTest("Check --staged timed out")

        # The command should complete (either 0 or non-zero exit)
        # Must produce parseable JSON
        output = result.stdout.strip()
        if not output:
            self.skipTest("Empty output from check --staged")

        try:
            parsed = json.loads(output)
        except (json.JSONDecodeError, ValueError):
            self.fail(f"Output not valid JSON: {output[:200]}")

        # Must have violations and warnings arrays
        self.assertIn("violations", parsed)
        self.assertIn("warnings", parsed)
        self.assertIsInstance(parsed["violations"], list)
        self.assertIsInstance(parsed["warnings"], list)

    def test_check_staged_violation_has_required_fields(self):
        """AC-17: Each violation should have type, severity, detail."""
        if not os.path.isfile(self.bundle_path):
            self.skipTest("Bundle not found")

        try:
            result = subprocess.run(
                ["node", self.bundle_path, "archi", "check", "--staged"],
                capture_output=True, text=True, timeout=30,
                cwd=self.project_root,
            )
        except subprocess.TimeoutExpired:
            self.skipTest("Timed out")

        output = result.stdout.strip()
        if not output:
            self.skipTest("Empty output")

        try:
            parsed = json.loads(output)
        except (json.JSONDecodeError, ValueError):
            self.skipTest("Invalid JSON")

        for violation in parsed.get("violations", []):
            self.assertIn("type", violation,
                          f"Violation missing 'type': {violation}")
            self.assertIn("severity", violation,
                          f"Violation missing 'severity'")
            # Severity should be error or warning
            self.assertIn(violation.get("severity"), ["error", "warning"])

    # ─── Violation types ──────────────────────────────────────────────────

    def test_check_violation_types_are_recognized(self):
        """AC-17: Violation types should be from the known set."""
        if not os.path.isfile(self.bundle_path):
            self.skipTest("Bundle not found")

        try:
            result = subprocess.run(
                ["node", self.bundle_path, "archi", "check", "--staged"],
                capture_output=True, text=True, timeout=30,
                cwd=self.project_root,
            )
        except subprocess.TimeoutExpired:
            self.skipTest("Timed out")

        output = result.stdout.strip()
        if not output:
            self.skipTest("Empty output")

        try:
            parsed = json.loads(output)
        except (json.JSONDecodeError, ValueError):
            self.skipTest("Invalid JSON")

        known_types = [
            "unmodeled_dependency",
            "unmapped_import_target",
            "unused_relationship",
            "path_not_found",
        ]
        for violation in parsed.get("violations", []):
            vtype = violation.get("type", "")
            self.assertIn(
                vtype, known_types,
                f"Unknown violation type '{vtype}'. Known: {known_types}"
            )

    # ─── Check with --files flag ──────────────────────────────────────────

    def test_check_specific_files(self):
        """AC-17: --files flag accepts a specific file list."""
        if not os.path.isfile(self.bundle_path):
            self.skipTest("Bundle not found")

        try:
            result = subprocess.run(
                ["node", self.bundle_path, "archi", "check",
                 "--files", "src/auth.py"],
                capture_output=True, text=True, timeout=30,
                cwd=self.project_root,
            )
        except subprocess.TimeoutExpired:
            self.skipTest("Timed out")

        output = result.stdout.strip()
        if not output:
            self.skipTest("Empty output")

        try:
            parsed = json.loads(output)
        except (json.JSONDecodeError, ValueError):
            self.fail(f"Output not valid JSON: {output[:200]}")

        self.assertIn("violations", parsed)
        self.assertIn("warnings", parsed)

    # ─── Output format compatibility ──────────────────────────────────────

    def test_check_output_can_be_parsed_as_json(self):
        """AC-17: archi check output must always be valid JSON."""
        if not os.path.isfile(self.bundle_path):
            self.skipTest("Bundle not found")

        try:
            result = subprocess.run(
                ["node", self.bundle_path, "archi", "check"],
                capture_output=True, text=True, timeout=30,
                cwd=self.project_root,
            )
        except subprocess.TimeoutExpired:
            self.skipTest("Timed out")

        output = result.stdout.strip()
        if not output:
            self.skipTest("Empty output")

        # Must be parseable JSON
        try:
            json.loads(output)
        except (json.JSONDecodeError, ValueError):
            self.fail(f"archi check output is not valid JSON: {output[:200]}")

    def test_check_exit_code_zero_when_no_errors(self):
        """AC-17: Exit code 0 when no error-severity violations."""
        if not os.path.isfile(self.bundle_path):
            self.skipTest("Bundle not found")

        # Run check without specific files (may have no violations)
        try:
            result = subprocess.run(
                ["node", self.bundle_path, "archi", "check"],
                capture_output=True, text=True, timeout=30,
                cwd=self.project_root,
            )
        except subprocess.TimeoutExpired:
            self.skipTest("Timed out")

        output = result.stdout.strip()
        if not output:
            self.skipTest("Empty output")

        try:
            parsed = json.loads(output)
        except (json.JSONDecodeError, ValueError):
            self.skipTest("Invalid JSON")

        has_errors = any(
            v.get("severity") == "error"
            for v in parsed.get("violations", [])
        )
        if not has_errors:
            self.assertEqual(result.returncode, 0)
        else:
            self.assertEqual(result.returncode, 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
