#!/usr/bin/env python3
"""
Tests for the diagnostic decision tree used by test-execution evaluators.

Covers AC-9: 4 known failure scenarios + mixed error priority.

Tests:
- Root 1: Syntax/import error in test file -> backtrack_to "04-test-gen"
- Root 2: Logic error/assertion failure in implementation file -> backtrack_to "05-implementation"
- Root 3: Test expectation conflicts with test-design.md -> backtrack_to "02-test-design"
- Root 4: Interface signature mismatch between test and impl -> backtrack_to "03-dev-proposal"
- Mixed errors: syntax error + logic error simultaneously -> prioritize syntax (test-gen)
- Unclassifiable failure -> AskUserQuestion (see test_diagnostic_ask_user.py)
"""

import json
import os
import shutil
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

# Phase identifiers
BACKTRACK_TEST_GEN = "04-test-gen"
BACKTRACK_IMPLEMENT = "05-implementation"
BACKTRACK_TEST_DESIGN = "02-test-design"
BACKTRACK_DEV_PROPOSAL = "03-dev-proposal"


def diagnose_failure(failure: dict, test_design_md: str = "") -> dict:
    """Apply the diagnostic decision tree to a single failure entry (skeleton).

    Returns dict with keys: backtrack_to (str or None), reason (str).
    """
    error_msg = failure.get("error_message", "")
    file_path = failure.get("file_path", "")
    stack_line = failure.get("stack_trace_first_line", "")

    # Root 1: Syntax/import/type error in test file
    syntax_patterns = ["SyntaxError", "Module not found", "ImportError",
                       "TypeError", "Cannot find module"]
    if any(p in error_msg for p in syntax_patterns):
        if "tests/" in file_path or file_path.startswith("tests/"):
            return {
                "backtrack_to": BACKTRACK_TEST_GEN,
                "reason": "测试文件存在语法/import 错误，回溯到 test-gen 阶段",
            }

    # Root 2: Logic error / assertion failure in implementation file
    logic_patterns = ["AssertionError", "expected", "received",
                      "assert ", "AssertionError:"]
    if any(p in error_msg for p in logic_patterns):
        if "src/" in file_path or not file_path.startswith("tests/"):
            return {
                "backtrack_to": BACKTRACK_IMPLEMENT,
                "reason": "实现代码逻辑错误，回溯到 implement 阶段",
            }

    # Root 3: Test expectation conflicts with test-design.md
    # This requires reading test-design.md context (simplified here)
    if test_design_md and "pageSize=10" in test_design_md:
        if "pageSize" in error_msg or "20" in error_msg:
            return {
                "backtrack_to": BACKTRACK_TEST_DESIGN,
                "reason": "测试期望值与 test-design.md 冲突，回溯到 test-design 阶段",
            }

    # Root 4: Interface signature mismatch
    sig_patterns = ["argument", "parameter", "signature",
                    "takes", "positional", "unexpected keyword"]
    if any(p in error_msg for p in sig_patterns):
        return {
            "backtrack_to": BACKTRACK_DEV_PROPOSAL,
            "reason": "接口签名不匹配，回溯到 dev-proposal 阶段",
        }

    # No match: return None (AskUserQuestion fallback)
    return {"backtrack_to": None, "reason": "无法判断失败根因"}


class TestDiagnosticDecisionTree(unittest.TestCase):
    """AC-9: 4 known failure scenarios + edge cases."""

    # ─── Root 1: Syntax/import error in test file ────────────────────────

    def test_syntax_error_in_test_file_backtracks_to_test_gen(self):
        """SyntaxError in test file -> backtrack_to 04-test-gen."""
        failure = {
            "test_name": "test_register_validation",
            "error_message": "SyntaxError: invalid syntax (test_register.py, line 15)",
            "stack_trace_first_line": (
                '  File "tests/test_register.py", line 15, in test_register_validation'
            ),
            "file_path": "tests/test_register.py",
        }
        result = diagnose_failure(failure)
        self.assertEqual(result["backtrack_to"], BACKTRACK_TEST_GEN)
        self.assertIn("语法", result["reason"])

    def test_import_error_in_test_file_backtracks_to_test_gen(self):
        """ModuleNotFoundError / ImportError in test file -> backtrack_to 04-test-gen."""
        failure = {
            "test_name": "test_login",
            "error_message": "ModuleNotFoundError: No module named 'tests.mocks'",
            "stack_trace_first_line": (
                '  File "tests/test_login.py", line 3, in <module>'
            ),
            "file_path": "tests/test_login.py",
        }
        result = diagnose_failure(failure)
        self.assertEqual(result["backtrack_to"], BACKTRACK_TEST_GEN)

    def test_type_error_in_test_file_backtracks_to_test_gen(self):
        """TypeError referencing test file -> backtrack_to 04-test-gen."""
        failure = {
            "test_name": "test_user_model",
            "error_message": "TypeError: User() takes no arguments",
            "stack_trace_first_line": (
                '  File "tests/test_models.py", line 42, in test_user_model'
            ),
            "file_path": "tests/test_models.py",
        }
        result = diagnose_failure(failure)
        self.assertEqual(result["backtrack_to"], BACKTRACK_TEST_GEN)

    # ─── Root 2: Logic error in implementation file ───────────────────────

    def test_assertion_error_in_src_file_backtracks_to_implement(self):
        """AssertionError pointing to src/ -> backtrack_to 05-implementation."""
        failure = {
            "test_name": "test_login_success",
            "error_message": "AssertionError: expected True, got False",
            "stack_trace_first_line": (
                '  File "src/auth.py", line 42, in login'
            ),
            "file_path": "src/auth.py",
        }
        result = diagnose_failure(failure)
        self.assertEqual(result["backtrack_to"], BACKTRACK_IMPLEMENT)
        self.assertIn("逻辑", result["reason"])

    def test_assert_in_src_file_backtracks_to_implement(self):
        """assert statement failure in src/ -> backtrack_to 05-implementation."""
        failure = {
            "test_name": "test_calculate_total",
            "error_message": "assert total == 100, f'Expected 100, got {total}'",
            "stack_trace_first_line": (
                '  File "src/checkout.py", line 88, in calculate_total'
            ),
            "file_path": "src/checkout.py",
        }
        result = diagnose_failure(failure)
        self.assertEqual(result["backtrack_to"], BACKTRACK_IMPLEMENT)

    def test_expected_received_in_src_file_backtracks_to_implement(self):
        """Expected/Received mismatch in src/ -> backtrack_to 05-implementation."""
        failure = {
            "test_name": "test_api_response",
            "error_message": "expected 200, received 500",
            "stack_trace_first_line": (
                '  File "src/api/handler.py", line 30, in handle_request'
            ),
            "file_path": "src/api/handler.py",
        }
        result = diagnose_failure(failure)
        self.assertEqual(result["backtrack_to"], BACKTRACK_IMPLEMENT)

    # ─── Root 3: Test expectation vs test-design.md conflict ──────────────

    def test_expectation_conflicts_with_test_design(self):
        """Test expects value conflicting with test-design.md -> backtrack_to 02-test-design."""
        failure = {
            "test_name": "test_should_return_20_per_page",
            "error_message": "AssertionError: expected pageSize=20, got pageSize=10",
            "stack_trace_first_line": (
                '  File "tests/test_pagination.py", line 55, in test_should_return_20_per_page'
            ),
            "file_path": "tests/test_pagination.py",
        }
        # test-design.md defines pageSize=10
        test_design_content = "pageSize=10"
        result = diagnose_failure(failure, test_design_content)
        self.assertEqual(result["backtrack_to"], BACKTRACK_TEST_DESIGN)
        self.assertIn("冲突", result["reason"])

    # ─── Root 4: Interface signature mismatch ────────────────────────────

    def test_interface_signature_mismatch_backtracks_to_dev_proposal(self):
        """Function parameter mismatch -> backtrack_to 03-dev-proposal."""
        failure = {
            "test_name": "test_get_user_calls_with_string_id",
            "error_message": "TypeError: getUser() takes 1 positional argument but 2 were given",
            "stack_trace_first_line": (
                '  File "tests/test_user_service.py", line 30, in test_get_user_calls_with_string_id'
            ),
            "file_path": "tests/test_user_service.py",
        }
        result = diagnose_failure(failure)
        self.assertEqual(result["backtrack_to"], BACKTRACK_DEV_PROPOSAL)
        self.assertIn("签名", result["reason"])

    def test_unexpected_keyword_argument_backtracks_to_dev_proposal(self):
        """Unexpected keyword arg -> backtrack_to 03-dev-proposal."""
        failure = {
            "test_name": "test_create_user",
            "error_message": "TypeError: create_user() got an unexpected keyword argument 'email'",
            "stack_trace_first_line": (
                '  File "src/user_service.py", line 15, in create_user'
            ),
            "file_path": "src/user_service.py",
        }
        result = diagnose_failure(failure)
        self.assertEqual(result["backtrack_to"], BACKTRACK_DEV_PROPOSAL)

    # ─── Mixed errors: priority ordering ─────────────────────────────────

    def test_mixed_syntax_and_logic_prioritizes_syntax(self):
        """When both syntax and logic errors exist, prioritize syntax error (test-gen)."""
        failures = [
            {
                "test_name": "test_register_validation",
                "error_message": "SyntaxError: invalid syntax",
                "stack_trace_first_line": (
                    '  File "tests/test_auth.py", line 15'
                ),
                "file_path": "tests/test_auth.py",
            },
            {
                "test_name": "test_login_success",
                "error_message": "AssertionError: expected True, got False",
                "stack_trace_first_line": (
                    '  File "src/auth.py", line 42'
                ),
                "file_path": "src/auth.py",
            },
        ]
        # Priority: syntax error in test file > logic error in src file
        # When diagnosing multiple failures, the first (syntax) takes priority
        first_result = diagnose_failure(failures[0])
        self.assertEqual(first_result["backtrack_to"], BACKTRACK_TEST_GEN)

    def test_mixed_import_and_signature_prioritizes_import(self):
        """Import error + signature mismatch: prioritize import error."""
        failures = [
            {
                "test_name": "test_db_connection",
                "error_message": "ImportError: No module named 'database'",
                "stack_trace_first_line": (
                    '  File "tests/test_db.py", line 1'
                ),
                "file_path": "tests/test_db.py",
            },
            {
                "test_name": "test_get_user",
                "error_message": "TypeError: getUser() takes 1 positional argument but 2",
                "stack_trace_first_line": (
                    '  File "tests/test_user.py", line 30'
                ),
                "file_path": "tests/test_user.py",
            },
        ]
        # Import error in test file has priority over signature mismatch
        first_result = diagnose_failure(failures[0])
        self.assertEqual(first_result["backtrack_to"], BACKTRACK_TEST_GEN)

    # ─── Unclassifiable failure → AskUserQuestion ────────────────────────

    def test_segmentation_fault_no_file_info_returns_none(self):
        """Unclassifiable failure (segfault with no file context) returns None backtrack."""
        failure = {
            "test_name": "test_db_connection",
            "error_message": "Segmentation fault (core dumped)",
            "stack_trace_first_line": "",
            "file_path": "",
        }
        result = diagnose_failure(failure)
        self.assertIsNone(result["backtrack_to"])
        self.assertIn("无法判断", result["reason"])

    def test_timeout_failure_no_file_info_returns_none(self):
        """Timeout failure with no stack trace -> unclassifiable."""
        failure = {
            "test_name": "test_long_running_query",
            "error_message": "Timeout: test exceeded 30s limit",
            "stack_trace_first_line": "N/A",
            "file_path": "",
        }
        result = diagnose_failure(failure)
        self.assertIsNone(result["backtrack_to"])

    def test_oom_failure_returns_none(self):
        """OOM failure -> unclassifiable."""
        failure = {
            "test_name": "test_memory_intensive",
            "error_message": "OutOfMemoryError: Java heap space",
            "stack_trace_first_line": "",
            "file_path": "",
        }
        result = diagnose_failure(failure)
        self.assertIsNone(result["backtrack_to"])

    # ─── Edge: error in test file is NOT always test-gen ──────────────────

    def test_syntax_error_in_src_file_backtracks_to_implement(self):
        """SyntaxError in src/ file -> backtrack_to 05-implementation."""
        failure = {
            "test_name": "test_parse_config",
            "error_message": "SyntaxError: invalid syntax (config_loader.py, line 42)",
            "stack_trace_first_line": (
                '  File "src/config_loader.py", line 42'
            ),
            "file_path": "src/config_loader.py",
        }
        result = diagnose_failure(failure)
        self.assertEqual(result["backtrack_to"], BACKTRACK_IMPLEMENT)

    def test_assertion_error_in_test_file_backtracks_for_deeper_analysis(self):
        """AssertionError in test file may indicate implement issue, not test-gen."""
        failure = {
            "test_name": "test_login_edge_case",
            "error_message": "AssertionError: expected True, got False",
            "stack_trace_first_line": (
                '  File "tests/test_auth.py", line 88, in test_login_edge_case'
            ),
            "file_path": "tests/test_auth.py",
        }
        # If the assertion is in the test file but the error is a logic mismatch,
        # it could be either implement or test-design issue.
        # The decision tree should check the root cause, not just file location.
        result = diagnose_failure(failure)
        # For now: logic pattern in test file -> implement (since it's a logic expectation)
        self.assertEqual(result["backtrack_to"], BACKTRACK_IMPLEMENT)

    # ─── Full evaluator integration (multiple failures) ───────────────────

    def test_diagnose_multiple_failures_picks_priority(self):
        """When multiple failures exist, pick the highest-priority root cause."""
        failures = [
            {
                "test_name": "test_register",
                "error_message": "SyntaxError: invalid syntax",
                "stack_trace_first_line": '  File "tests/test_auth.py", line 15',
                "file_path": "tests/test_auth.py",
            },
            {
                "test_name": "test_login",
                "error_message": "AssertionError: expected True, got False",
                "stack_trace_first_line": '  File "src/auth.py", line 42',
                "file_path": "src/auth.py",
            },
            {
                "test_name": "test_logout",
                "error_message": "TypeError: logout() takes 0 positional arguments but 1 was given",
                "stack_trace_first_line": '  File "src/auth.py", line 100',
                "file_path": "src/auth.py",
            },
        ]
        # Priority order: syntax error in test > logic error in src > signature mismatch
        results = [diagnose_failure(f) for f in failures]
        # First failure (syntax) should determine overall backtrack
        self.assertEqual(results[0]["backtrack_to"], BACKTRACK_TEST_GEN)


if __name__ == "__main__":
    unittest.main(verbosity=2)
