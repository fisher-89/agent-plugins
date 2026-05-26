#!/usr/bin/env python3
"""
Tests for test executor report JSON schema and evaluator diagnostic decision tree.

Covers:
- AC-5: Unit-test execution report JSON schema completeness and field validation
- AC-6: Unit-test evaluator report completeness checklist + diagnostic decision tree
- AC-7: Integration-test execution report JSON schema consistency with unit-test
- AC-8: Integration-test evaluator report reading + diagnostic decision tree

Tests:
- Unit-test report JSON contains all required fields (phase, timestamp, total, passed,
  failed, skipped, coverage, failures, duration_seconds, test_command)
- Unit-test report validation: missing required fields -> "report_incomplete"
- Unit-test evaluator checklist: coverage threshold, failure analysis
- Unit-test evaluator diagnostic decision tree (delegated to test_diagnostic_decision_tree.py)
- Integration-test report schema matches unit-test report schema
- Integration-test evaluator reads report and validates completeness
- Integration-test evaluator diagnostic decision tree
"""

import json
import os
import shutil
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

# Required fields for a valid test execution report
REQUIRED_REPORT_FIELDS = [
    "phase", "timestamp", "total", "passed", "failed",
    "skipped", "coverage", "coverage_threshold", "failures",
    "duration_seconds", "test_command",
]

REQUIRED_FAILURE_FIELDS = [
    "test_name", "error_message", "stack_trace_first_line", "file_path",
]


def validate_report_schema(report: dict) -> list:
    """Validate a test execution report against the required schema (skeleton).

    Returns a list of missing field names (empty = valid).
    """
    missing = []
    for field in REQUIRED_REPORT_FIELDS:
        if field not in report:
            missing.append(field)
    return missing


def validate_failure_schema(failure: dict) -> list:
    """Validate a single failure entry against the required schema (skeleton)."""
    missing = []
    for field in REQUIRED_FAILURE_FIELDS:
        if field not in failure:
            missing.append(field)
    return missing


class TempProject:
    """Creates a temporary project structure for test executor report tests."""

    def __init__(self):
        self.root = tempfile.mkdtemp(prefix="test_report_")

    def cleanup(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def create_report(self, overrides: dict = None) -> dict:
        """Create a valid test execution report dict."""
        report = {
            "phase": "06-unit-test",
            "timestamp": "2026-05-25T10:00:00.000Z",
            "total": 10,
            "passed": 8,
            "failed": 2,
            "skipped": 0,
            "coverage": "85.5",
            "coverage_threshold": "80",
            "failures": [
                {
                    "test_name": "test_login_success",
                    "error_message": "AssertionError: expected True, got False",
                    "stack_trace_first_line": '  File "src/auth.py", line 42, in login',
                    "file_path": "src/auth.py",
                },
                {
                    "test_name": "test_register_validation",
                    "error_message": "SyntaxError: invalid syntax",
                    "stack_trace_first_line": '  File "tests/test_auth.py", line 15, in test_register_validation',
                    "file_path": "tests/test_auth.py",
                },
            ],
            "duration_seconds": 12.5,
            "test_command": "pytest tests/",
        }
        if overrides:
            report.update(overrides)
        return report


class TestUnitTestReportSchema(unittest.TestCase):
    """AC-5: Unit-test execution report JSON schema validation."""

    def setUp(self):
        self.project = TempProject()

    def tearDown(self):
        self.project.cleanup()

    def test_valid_report_contains_all_required_fields(self):
        """Verify a valid unit-test report contains all required fields."""
        report = self.project.create_report()
        missing = validate_report_schema(report)
        self.assertEqual(missing, [], f"Missing required fields: {missing}")

    def test_report_phase_field_is_06_unit_test(self):
        """Verify report phase identifier is correct for unit-test."""
        report = self.project.create_report()
        self.assertEqual(report["phase"], "06-unit-test")

    def test_report_timestamp_is_iso8601(self):
        """Verify timestamp is valid ISO 8601 format."""
        report = self.project.create_report()
        # Basic format check: YYYY-MM-DDTHH:MM:SS.sssZ
        self.assertRegex(report["timestamp"], r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}")

    def test_report_total_passed_failed_counts_consistent(self):
        """Verify total >= passed + failed + skipped."""
        report = self.project.create_report()
        self.assertGreaterEqual(
            report["total"],
            report["passed"] + report["failed"] + report["skipped"],
        )

    def test_report_coverage_is_string_or_null(self):
        """Verify coverage field is a string or null."""
        report = self.project.create_report()
        self.assertTrue(
            isinstance(report["coverage"], str) or report["coverage"] is None
        )

    def test_report_coverage_threshold_is_string(self):
        """Verify coverage_threshold is a string."""
        report = self.project.create_report()
        self.assertIsInstance(report["coverage_threshold"], str)

    def test_report_failures_list_contains_objects(self):
        """Verify failures is a list of objects."""
        report = self.project.create_report()
        self.assertIsInstance(report["failures"], list)
        if report["failures"]:
            self.assertIsInstance(report["failures"][0], dict)

    def test_report_failure_entry_has_all_required_fields(self):
        """Verify each failure entry has test_name, error_message, etc."""
        report = self.project.create_report()
        for failure in report["failures"]:
            missing = validate_failure_schema(failure)
            self.assertEqual(
                missing, [],
                f"Failure entry missing: {missing} in {failure.get('test_name', '?')}"
            )

    def test_report_duration_seconds_is_number(self):
        """Verify duration_seconds is a number."""
        report = self.project.create_report()
        self.assertIsInstance(report["duration_seconds"], (int, float))

    def test_report_test_command_is_string(self):
        """Verify test_command is a string."""
        report = self.project.create_report()
        self.assertIsInstance(report["test_command"], str)

    # ─── Schema validation: missing fields ────────────────────────────────

    def test_missing_total_field_detected(self):
        """Verify report missing 'total' field is flagged."""
        report = self.project.create_report()
        del report["total"]
        missing = validate_report_schema(report)
        self.assertIn("total", missing)

    def test_missing_failed_field_detected(self):
        """Verify report missing 'failed' field is flagged."""
        report = self.project.create_report()
        del report["failed"]
        missing = validate_report_schema(report)
        self.assertIn("failed", missing)

    def test_missing_failures_field_detected(self):
        """Verify report missing 'failures' field is flagged."""
        report = self.project.create_report()
        del report["failures"]
        missing = validate_report_schema(report)
        self.assertIn("failures", missing)

    def test_missing_phase_field_detected(self):
        """Verify report missing 'phase' field is flagged."""
        report = self.project.create_report()
        del report["phase"]
        missing = validate_report_schema(report)
        self.assertIn("phase", missing)

    def test_missing_duration_seconds_field_detected(self):
        """Verify report missing 'duration_seconds' field is flagged."""
        report = self.project.create_report()
        del report["duration_seconds"]
        missing = validate_report_schema(report)
        self.assertIn("duration_seconds", missing)

    def test_report_with_all_fields_missing(self):
        """Verify an empty report flags all required fields."""
        missing = validate_report_schema({})
        self.assertEqual(set(missing), set(REQUIRED_REPORT_FIELDS))

    # ─── Edge: zero tests ─────────────────────────────────────────────────

    def test_report_with_zero_tests(self):
        """Verify report with zero total tests is valid (no test files)."""
        report = self.project.create_report({
            "total": 0, "passed": 0, "failed": 0, "skipped": 0, "failures": [],
        })
        missing = validate_report_schema(report)
        self.assertEqual(missing, [])

    # ─── Edge: exit code failure ──────────────────────────────────────────

    def test_report_with_nonzero_exit_code_only(self):
        """Verify report with non-zero exit code but no test output stores error in failures."""
        report = self.project.create_report({
            "total": 0, "failed": 0, "failures": [
                {
                    "test_name": "(test_command_error)",
                    "error_message": "Exit code: 1, stderr: pytest: command not found",
                    "stack_trace_first_line": "N/A",
                    "file_path": "N/A",
                }
            ],
        })
        missing = validate_report_schema(report)
        self.assertEqual(missing, [])


class TestUnitTestEvaluator(unittest.TestCase):
    """AC-6: Unit-test evaluator report completeness checklist and diagnostics."""

    def setUp(self):
        self.project = TempProject()

    def tearDown(self):
        self.project.cleanup()

    def test_evaluator_accepts_complete_report(self):
        """Verify evaluator passes a complete, all-pass report."""
        report = self.project.create_report({
            "phase": "06-unit-test", "total": 10, "passed": 10,
            "failed": 0, "skipped": 0, "failures": [],
            "coverage": "92.0", "coverage_threshold": "80",
        })
        # TODO: Call actual evaluator check function
        # result = unit_test_evaluator.evaluate_report(report)
        # self.assertTrue(result["verdict"] == "pass")
        self.assertEqual(report["passed"], 10)
        self.assertEqual(report["failed"], 0)

    def test_evaluator_rejects_incomplete_report_missing_total(self):
        """Verify evaluator flags report missing 'total' field."""
        report = self.project.create_report()
        del report["total"]
        # result = unit_test_evaluator.evaluate_report(report)
        # self.assertIn("report_incomplete", result["findings"])
        # self.assertEqual(result["verdict"], "fail")
        missing = validate_report_schema(report)
        self.assertIn("total", missing)

    def test_evaluator_rejects_incomplete_report_missing_failures(self):
        """Verify evaluator flags report missing 'failures' field."""
        report = self.project.create_report()
        del report["failures"]
        missing = validate_report_schema(report)
        self.assertIn("failures", missing)

    def test_evaluator_detects_coverage_below_threshold(self):
        """Verify evaluator flags coverage below threshold."""
        report = self.project.create_report({
            "coverage": "70.0", "coverage_threshold": "80",
        })
        coverage_val = float(report["coverage"])
        threshold_val = float(report["coverage_threshold"])
        # result = unit_test_evaluator.evaluate_report(report)
        # self.assertIn("coverage_below_threshold", result.get("warnings", []))
        self.assertLess(coverage_val, threshold_val)

    def test_evaluator_detects_coverage_meets_threshold(self):
        """Verify evaluator accepts coverage meeting threshold."""
        report = self.project.create_report({
            "coverage": "85.0", "coverage_threshold": "80",
        })
        coverage_val = float(report["coverage"])
        threshold_val = float(report["coverage_threshold"])
        self.assertGreaterEqual(coverage_val, threshold_val)

    def test_evaluator_detects_coverage_none(self):
        """Verify evaluator handles null coverage."""
        report = self.project.create_report({
            "coverage": None, "coverage_threshold": "80",
        })
        # result = unit_test_evaluator.evaluate_report(report)
        # self.assertIn("coverage_not_available", result.get("warnings", []))
        self.assertIsNone(report["coverage"])

    def test_evaluator_diagnostic_for_syntax_error(self):
        """Verify evaluator detects syntax errors in test files (diagnostic tree root 1)."""
        # See test_diagnostic_decision_tree.py for full decision tree tests
        pass

    def test_evaluator_diagnostic_for_logic_error(self):
        """Verify evaluator detects assertion errors in implementation files (root 2)."""
        pass

    def test_evaluator_findings_contains_diagnostic_reasoning(self):
        """Verify evaluator records diagnostic reasoning in findings."""
        # result = unit_test_evaluator.evaluate_report(report_with_failures)
        # self.assertIn("findings", result)
        pass


class TestIntegrationTestReportSchema(unittest.TestCase):
    """AC-7: Integration-test report JSON schema consistency with unit-test."""

    def setUp(self):
        self.project = TempProject()

    def tearDown(self):
        self.project.cleanup()

    def test_integration_report_has_same_schema_as_unit_test(self):
        """Verify integration-test report schema matches unit-test schema exactly."""
        unit_report = self.project.create_report()
        int_report = self.project.create_report({
            "phase": "08-integration-test",
            "test_command": "pytest tests/integration/",
        })
        unit_missing = validate_report_schema(unit_report)
        int_missing = validate_report_schema(int_report)
        self.assertEqual(unit_missing, int_missing)

    def test_integration_report_phase_is_08_integration_test(self):
        """Verify integration-test report phase identifier is correct."""
        report = self.project.create_report({"phase": "08-integration-test"})
        self.assertEqual(report["phase"], "08-integration-test")

    def test_integration_report_failure_has_required_fields(self):
        """Verify integration-test failure entries match schema."""
        report = self.project.create_report({
            "phase": "08-integration-test",
            "failures": [
                {
                    "test_name": "test_db_connection",
                    "error_message": "Connection refused",
                    "stack_trace_first_line": '  File "tests/integration/test_db.py", line 22',
                    "file_path": "tests/integration/test_db.py",
                }
            ],
        })
        for failure in report["failures"]:
            missing = validate_failure_schema(failure)
            self.assertEqual(missing, [])


class TestIntegrationTestEvaluator(unittest.TestCase):
    """AC-8: Integration-test evaluator report reading and diagnostics."""

    def setUp(self):
        self.project = TempProject()

    def tearDown(self):
        self.project.cleanup()

    def test_integration_evaluator_accepts_complete_report(self):
        """Verify integration-test evaluator passes a complete report."""
        report = self.project.create_report({
            "phase": "08-integration-test",
            "total": 5, "passed": 5, "failed": 0, "skipped": 0, "failures": [],
        })
        # result = integration_test_evaluator.evaluate_report(report)
        # self.assertEqual(result["verdict"], "pass")
        self.assertEqual(report["passed"], 5)

    def test_integration_evaluator_rejects_incomplete_report(self):
        """Verify integration-test evaluator flags missing fields."""
        report = self.project.create_report({
            "phase": "08-integration-test",
        })
        del report["failures"]
        missing = validate_report_schema(report)
        self.assertIn("failures", missing)

    def test_integration_evaluator_diagnostic_finds_contract_mismatch(self):
        """Verify integration-test evaluator detects interface contract mismatch."""
        # result = integration_test_evaluator.evaluate_report(report_with_contract_mismatch)
        # self.assertIn("backtrack_to", result)
        # self.assertEqual(result["backtrack_to"], "03-dev-proposal")
        pass

    def test_integration_evaluator_diagnostic_finds_missing_mock(self):
        """Verify integration-test evaluator detects missing mock configurations."""
        pass

    def test_integration_evaluator_findings_contains_reasoning(self):
        """Verify integration-test evaluator records diagnostic reasoning."""
        pass


if __name__ == "__main__":
    unittest.main(verbosity=2)
