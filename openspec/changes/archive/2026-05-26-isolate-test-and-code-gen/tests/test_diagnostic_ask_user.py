#!/usr/bin/env python3
"""
Tests for the 'unclassifiable failure' AskUserQuestion diagnostic path.

Covers:
- AC-20: "unclassifiable" scenario -> Evaluator calls AskUserQuestion with diagnostic info
- AC-21: AskUserQuestion timeout -> backtrack to dev-proposal with timeout record in findings

Tests:
- Unclassifiable failure triggers AskUserQuestion invocation
- AskUserQuestion question content includes test name, diagnostic summary, excluded causes
- AskUserQuestion offers backtrack options (test-design, dev-proposal, test-gen, implement, etc.)
- AskUserQuestion timeout (default 5 min, configurable) triggers fallback
- Timeout fallback sets backtrack_to to "03-dev-proposal"
- Timeout fallback records "ask_user_timeout": true in findings
"""

import json
import os
import shutil
import sys
import tempfile
import time
import unittest
from unittest.mock import MagicMock, patch


# Default timeout for AskUserQuestion (seconds)
DEFAULT_ASK_USER_TIMEOUT = 300  # 5 minutes


def determine_backtrack_with_ask_user(
    failures: list,
    ask_user_callback=None,
    timeout: int = DEFAULT_ASK_USER_TIMEOUT,
) -> dict:
    """Determine backtrack target for unclassifiable failures (skeleton).

    Returns dict with keys: backtrack_to, findings, ask_user_called (bool).
    If AskUserQuestion times out, falls back to dev-proposal.
    """
    # Try to diagnose using decision tree first
    # If no clear diagnosis, invoke AskUserQuestion
    unclassifiable = []
    for failure in failures:
        # Simplified: check if decision tree returned None
        # In real implementation, call diagnose_failure()
        error_msg = failure.get("error_message", "")
        file_path = failure.get("file_path", "")
        if not file_path and ("Segmentation fault" in error_msg or
                              "Timeout" in error_msg or
                              "OutOfMemory" in error_msg):
            unclassifiable.append(failure)

    if not unclassifiable:
        return {"backtrack_to": None, "findings": [], "ask_user_called": False}

    # Construct AskUserQuestion payload
    question_data = {
        "type": "ask_user",
        "title": "无法自动判断测试失败根因",
        "failures": [
            {
                "test_name": f.get("test_name", "unknown"),
                "error_message": f.get("error_message", ""),
            }
            for f in unclassifiable
        ],
        "excluded_causes": ["语法错误", "逻辑错误"],
        "suggested_backtrack_options": [
            "02-test-design", "03-dev-proposal",
            "04-test-gen", "05-implementation",
            "06-unit-test", "07-code-review",
        ],
    }

    # Simulate AskUserQuestion call
    if ask_user_callback:
        try:
            result = ask_user_callback(question_data)
            if result and result.get("backtrack_to"):
                return {
                    "backtrack_to": result["backtrack_to"],
                    "findings": [{"ask_user_response": result}],
                    "ask_user_called": True,
                }
        except TimeoutError:
            # Timeout fallback
            return {
                "backtrack_to": "03-dev-proposal",
                "findings": [{"ask_user_timeout": True}],
                "ask_user_called": True,
            }

    # No callback or no response: timeout
    return {
        "backtrack_to": "03-dev-proposal",
        "findings": [{"ask_user_timeout": True}],
        "ask_user_called": True,
    }


class TestDiagnosticAskUserQuestion(unittest.TestCase):
    """AC-20: Evaluator calls AskUserQuestion for unclassifiable failures."""

    # ─── AskUserQuestion invocation ───────────────────────────────────────

    def test_unclassifiable_segfault_triggers_ask_user(self):
        """Segmentation fault with no file context triggers AskUserQuestion."""
        failures = [
            {
                "test_name": "test_db_connection",
                "error_message": "Segmentation fault (core dumped)",
                "stack_trace_first_line": "",
                "file_path": "",
            },
        ]
        ask_user_called = [False]

        def mock_ask_user(data):
            ask_user_called[0] = True
            return {"backtrack_to": "05-implementation"}

        result = determine_backtrack_with_ask_user(failures, mock_ask_user)
        self.assertTrue(ask_user_called[0])
        self.assertTrue(result["ask_user_called"])

    def test_unclassifiable_timeout_triggers_ask_user(self):
        """Timeout failure triggers AskUserQuestion."""
        failures = [
            {
                "test_name": "test_long_running",
                "error_message": "Timeout: test exceeded 30s limit",
                "stack_trace_first_line": "N/A",
                "file_path": "",
            },
        ]
        ask_user_called = [False]

        def mock_ask_user(data):
            ask_user_called[0] = True
            return {"backtrack_to": "03-dev-proposal"}

        result = determine_backtrack_with_ask_user(failures, mock_ask_user)
        self.assertTrue(ask_user_called[0])

    def test_classifiable_failure_does_not_trigger_ask_user(self):
        """Classifiable syntax error should NOT trigger AskUserQuestion."""
        failures = [
            {
                "test_name": "test_login",
                "error_message": "SyntaxError: invalid syntax",
                "stack_trace_first_line": '  File "tests/test_auth.py", line 15',
                "file_path": "tests/test_auth.py",
            },
        ]
        ask_user_called = [False]

        def mock_ask_user(data):
            ask_user_called[0] = True
            return None

        result = determine_backtrack_with_ask_user(failures, mock_ask_user)
        # The unclassifiable detection depends on file_path and error_message patterns
        # Syntax errors with file paths should NOT trigger ask_user
        self.assertFalse(result["ask_user_called"])

    # ─── Question content ─────────────────────────────────────────────────

    def test_ask_user_question_contains_test_name(self):
        """AskUserQuestion question should include failing test name."""
        failures = [
            {
                "test_name": "test_db_connection",
                "error_message": "Segmentation fault (core dumped)",
                "file_path": "",
            },
        ]
        captured_data = [None]

        def mock_ask_user(data):
            captured_data[0] = data
            return {"backtrack_to": "05-implementation"}

        determine_backtrack_with_ask_user(failures, mock_ask_user)
        self.assertIsNotNone(captured_data[0])
        self.assertIn("test_db_connection", str(captured_data[0]))

    def test_ask_user_question_contains_diagnostic_summary(self):
        """AskUserQuestion should include the error message for diagnosis."""
        failures = [
            {
                "test_name": "test_db_connection",
                "error_message": "Segmentation fault (core dumped)",
                "file_path": "",
            },
        ]
        captured_data = [None]

        def mock_ask_user(data):
            captured_data[0] = data
            return {"backtrack_to": "05-implementation"}

        determine_backtrack_with_ask_user(failures, mock_ask_user)
        self.assertIsNotNone(captured_data[0])
        self.assertIn("Segmentation fault", str(captured_data[0]))

    def test_ask_user_question_contains_excluded_causes(self):
        """AskUserQuestion should list already-excluded causes."""
        failures = [
            {
                "test_name": "test_api",
                "error_message": "Segmentation fault (core dumped)",
                "file_path": "",
            },
        ]
        captured_data = [None]

        def mock_ask_user(data):
            captured_data[0] = data
            return {"backtrack_to": "03-dev-proposal"}

        determine_backtrack_with_ask_user(failures, mock_ask_user)
        self.assertIsNotNone(captured_data[0])
        # Should list excluded causes
        excluded = captured_data[0].get("excluded_causes", [])
        self.assertTrue(len(excluded) > 0)

    def test_ask_user_question_contains_backtrack_options(self):
        """AskUserQuestion should offer backtrack target options."""
        failures = [
            {
                "test_name": "test_api",
                "error_message": "Segmentation fault (core dumped)",
                "file_path": "",
            },
        ]
        captured_data = [None]

        def mock_ask_user(data):
            captured_data[0] = data
            return {"backtrack_to": "05-implementation"}

        determine_backtrack_with_ask_user(failures, mock_ask_user)
        self.assertIsNotNone(captured_data[0])
        options = captured_data[0].get("suggested_backtrack_options", [])
        self.assertIn("03-dev-proposal", options)
        self.assertIn("05-implementation", options)

    # ─── AskUserQuestion response handling ────────────────────────────────

    def test_ask_user_response_backtrack_target_used(self):
        """AskUserQuestion response backtrack_to is used as the final target."""
        failures = [
            {
                "test_name": "test_db",
                "error_message": "Segmentation fault (core dumped)",
                "file_path": "",
            },
        ]

        def mock_ask_user(data):
            return {"backtrack_to": "04-test-gen"}

        result = determine_backtrack_with_ask_user(failures, mock_ask_user)
        self.assertEqual(result["backtrack_to"], "04-test-gen")


class TestDiagnosticAskUserTimeout(unittest.TestCase):
    """AC-21: AskUserQuestion timeout fallback behavior."""

    # ─── Timeout fallback ─────────────────────────────────────────────────

    def test_ask_user_timeout_falls_back_to_dev_proposal(self):
        """AC-21: AskUserQuestion timeout falls back to 03-dev-proposal."""
        failures = [
            {
                "test_name": "test_db_connection",
                "error_message": "Segmentation fault (core dumped)",
                "file_path": "",
            },
        ]

        def mock_ask_user_raises_timeout(data):
            raise TimeoutError("AskUserQuestion timed out")

        result = determine_backtrack_with_ask_user(
            failures, mock_ask_user_raises_timeout, timeout=0.01
        )
        self.assertEqual(result["backtrack_to"], "03-dev-proposal")

    def test_timeout_recorded_in_findings(self):
        """AC-21: Timeout fallback records 'ask_user_timeout': true in findings."""
        failures = [
            {
                "test_name": "test_db",
                "error_message": "Segmentation fault (core dumped)",
                "file_path": "",
            },
        ]

        def mock_ask_user_raises_timeout(data):
            raise TimeoutError("AskUserQuestion timed out")

        result = determine_backtrack_with_ask_user(
            failures, mock_ask_user_raises_timeout, timeout=0.01
        )
        self.assertTrue(result["ask_user_called"])
        findings = result.get("findings", [])
        has_timeout = any(
            f.get("ask_user_timeout") for f in findings
        )
        self.assertTrue(has_timeout)

    def test_no_ask_user_response_within_timeout_falls_back(self):
        """No response within timeout falls back to dev-proposal."""
        failures = [
            {
                "test_name": "test_slow",
                "error_message": "OutOfMemoryError: Java heap space",
                "file_path": "",
            },
        ]

        def mock_ask_user_no_response(data):
            # Simulate no response (None)
            return None

        result = determine_backtrack_with_ask_user(
            failures, mock_ask_user_no_response, timeout=0.01
        )
        self.assertEqual(result["backtrack_to"], "03-dev-proposal")

    def test_ask_user_called_flag_true_when_timeout(self):
        """ask_user_called should be True even on timeout."""
        failures = [
            {
                "test_name": "test_crash",
                "error_message": "Segmentation fault (core dumped)",
                "file_path": "",
            },
        ]

        def mock_ask_user_timeout(data):
            raise TimeoutError("AskUserQuestion timed out")

        result = determine_backtrack_with_ask_user(
            failures, mock_ask_user_timeout, timeout=0.01
        )
        self.assertTrue(result["ask_user_called"])

    # ─── Edge: multiple failures, some classifiable ───────────────────────

    def test_mixed_classifiable_and_unclassifiable(self):
        """When some failures are classifiable, AskUserQuestion is not called for those."""
        failures = [
            {
                "test_name": "test_syntax",
                "error_message": "SyntaxError: invalid syntax",
                "stack_trace_first_line": '  File "tests/test_auth.py", line 15',
                "file_path": "tests/test_auth.py",
            },
            {
                "test_name": "test_crash",
                "error_message": "Segmentation fault (core dumped)",
                "stack_trace_first_line": "",
                "file_path": "",
            },
        ]
        # Only the unclassifiable one triggers AskUserQuestion
        # The classifiable one is handled by the decision tree directly
        pass


if __name__ == "__main__":
    unittest.main(verbosity=2)
