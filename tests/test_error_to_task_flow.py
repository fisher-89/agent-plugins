#!/usr/bin/env python3
"""
Integration test for ERROR→Task flow.

Tests that when a code review finds ERRORs:
1. The hook injects actionable instructions
2. Running review-parser generates fix tasks
3. Fix tasks are appended to tasks.md
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest

# Add plugin utils to path
PLUGIN_ROOT = os.path.join(os.path.dirname(__file__), "..", "plugin")
UTILS_DIR = os.path.join(PLUGIN_ROOT, "utils")
if UTILS_DIR not in sys.path:
    sys.path.insert(0, UTILS_DIR)

# Import report_chain module directly
import importlib.util
_rc_path = os.path.join(UTILS_DIR, "report-chain.py")
if os.path.isfile(_rc_path):
    _rc_spec = importlib.util.spec_from_file_location("report_chain", _rc_path)
    report_chain = importlib.util.module_from_spec(_rc_spec)
    _rc_spec.loader.exec_module(report_chain)


class TestErrorToTaskFlow(unittest.TestCase):
    """Test ERROR→Task flow integration."""

    def setUp(self):
        """Set up test change directory."""
        self.temp_dir = tempfile.mkdtemp()
        self.change_dir = os.path.join(self.temp_dir, "openspec", "changes", "test-change")
        os.makedirs(self.change_dir, exist_ok=True)

        # Create tasks.md
        self.tasks_md = os.path.join(self.change_dir, "tasks.md")
        with open(self.tasks_md, "w", encoding="utf-8") as f:
            f.write("""# Tasks: test-change

## Phase 1

- [x] Task 1 complete

- [ ] Task 2 pending
""")

        # Create test-reports directory
        self.test_reports_dir = os.path.join(self.change_dir, "test-reports")
        os.makedirs(self.test_reports_dir, exist_ok=True)

        # Create step reports directory (for report chain check)
        self.reports_dir = os.path.join(self.change_dir, "reports")
        os.makedirs(self.reports_dir, exist_ok=True)

        # Create required step reports for task 2 (current task)
        # Timestamps must be strictly increasing: scope < lint < scoped-test
        step_timestamps = {
            "scope": "2026-05-08T12:00:01.000Z",
            "lint": "2026-05-08T12:00:02.000Z",
            "scoped-test": "2026-05-08T12:00:03.000Z",
        }
        for step in ["scope", "lint", "scoped-test"]:
            report_path = os.path.join(self.reports_dir, f"task-2_{step}.json")
            with open(report_path, "w", encoding="utf-8") as f:
                json.dump({
                    "schema": "openspec-step-report/v1",
                    "change": "test-change",
                    "task_id": "2",
                    "step": step,
                    "status": "pass",
                    "timestamp": step_timestamps[step],
                    "details": {},
                }, f)

        # Create a review report with ERRORs
        self.review_path = os.path.join(self.test_reports_dir, "code-review-20260508-120000.md")
        with open(self.review_path, "w", encoding="utf-8") as f:
            f.write("""# Code Review

## Summary
Found 2 errors.

## Verdict
**BLOCK**

## Errors

#### [ERROR-1] src/auth.py:45
- **Category**: Logical Error
- **Description**: Missing null check for user input
- **Current Code**: `password = user.password`
- **Suggested Fix**: `password = user.password if user else None`

#### [ERROR-2] src/db.py:102
- **Category**: Performance
- **Description**: Unoptimized database query
- **Current Code**: `query = f"SELECT * FROM users WHERE id = {user_id}"`
- **Suggested Fix**: Use parameterized query
""")

    def tearDown(self):
        """Clean up test directory."""
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_commit_hook_injects_error_to_task_instruction(self):
        """Test that commit hook injects ERROR→Task instruction."""
        # Import hook module
        hook_path = os.path.join(PLUGIN_ROOT, "hooks", "pre-tool-commit-review.py")

        # Simulate hook input
        hook_input = {
            "tool_name": "Bash",
            "tool_input": {"command": "git commit -m 'test'"},
            "cwd": self.temp_dir,
        }

        # Run hook
        result = subprocess.run(
            [sys.executable, hook_path],
            input=json.dumps(hook_input),
            capture_output=True,
            text=True,
            timeout=10,
        )

        output = json.loads(result.stdout)
        context = output.get("hookSpecificOutput", {}).get("additionalContext", "")

        # Should contain ERROR→Task instructions
        self.assertIn("REVIEW BLOCK", context)
        self.assertIn("2 error(s)", context)
        self.assertIn("Generate fix tasks", context)
        self.assertIn("review-parser.py", context)
        self.assertIn("Re-enter apply", context)

    def test_review_parser_generates_fix_tasks(self):
        """Test that review-parser generates fix tasks."""
        # Check if review-parser exists
        parser_path = os.path.join(UTILS_DIR, "review-parser.py")
        if not os.path.isfile(parser_path):
            self.skipTest("review-parser.py not available")

        # Run review-parser
        result = subprocess.run(
            [
                sys.executable, parser_path,
                self.review_path,
                "--generate-tasks",
                "--append-to", self.tasks_md,
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )

        # Read updated tasks.md
        with open(self.tasks_md, "r", encoding="utf-8") as f:
            content = f.read()

        # Should have fix tasks appended
        self.assertIn("Fix:", content)
        # Should have 2 fix tasks (one for each ERROR)
        self.assertGreaterEqual(content.count("Fix:"), 2)

    def test_report_chain_checker(self):
        """Test report chain checker."""
        # Test with no reports for task 3 - should fail
        ok, err = report_chain.check_report_chain(self.change_dir, "3")
        self.assertFalse(ok)
        self.assertIn("Missing report", err)

        # Test with all reports for task 2 - should pass
        ok, err = report_chain.check_report_chain(self.change_dir, "2")
        self.assertTrue(ok)

        # Test parse_tasks
        tasks = report_chain.parse_tasks(self.tasks_md)
        self.assertEqual(len(tasks), 2)
        self.assertEqual(tasks[0][0], "1")  # task_id
        self.assertTrue(tasks[0][2])  # completed
        self.assertFalse(tasks[1][2])  # not completed


if __name__ == "__main__":
    unittest.main()
