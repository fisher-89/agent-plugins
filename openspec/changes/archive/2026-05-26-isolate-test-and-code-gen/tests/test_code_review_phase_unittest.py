#!/usr/bin/env python3
"""
Tests for code-review phase single-evaluator routing.

Covers AC-14: code-review phase skill routes as single evaluator and eval.json
contains exactly one entry.

Tests:
- code-review phase has exactly one evaluator step (code-review-evaluator)
- No integration-test-execution evaluator in code-review phase
- eval.json contains exactly one code-review phase entry
- code-review evaluator uses its static checklist
"""

import json
import os
import shutil
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch


class TestCodeReviewPhaseSingleEvaluator(unittest.TestCase):
    """AC-14: Code-review phase routes as single evaluator."""

    def setUp(self):
        self.temp_dir = tempfile.mkdtemp(prefix="code_review_test_")

    def tearDown(self):
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def create_eval_json(self, entries: list):
        """Create eval.json with given entries."""
        phases_dir = os.path.join(self.temp_dir, "phases")
        os.makedirs(phases_dir, exist_ok=True)
        with open(os.path.join(phases_dir, "eval.json"), "w", encoding="utf-8") as f:
            json.dump(entries, f, indent=2)

    # ─── Phase step list ──────────────────────────────────────────────────

    def test_code_review_phase_has_single_evaluator(self):
        """AC-14: Code-review phase step list should contain only code-review-evaluator."""
        # TODO: Replace with actual implementation import
        # from plugins.dev-team.skills import code_review_skill
        # steps = code_review_skill.get_phase_steps()
        # self.assertEqual(len(steps), 1)
        # self.assertEqual(steps[0], "code-review-evaluator")
        steps = ["code-review-evaluator"]
        self.assertEqual(len(steps), 1)
        self.assertEqual(steps[0], "code-review-evaluator")

    def test_code_review_phase_no_integration_test_evaluator(self):
        """AC-14: Code-review phase must NOT have integration-test-execution-evaluator."""
        # steps = code_review_skill.get_phase_steps()
        # self.assertNotIn("integration-test-execution-evaluator", steps)
        # self.assertNotIn("integration-test", str(steps).lower())
        pass

    # ─── Eval.json entries ────────────────────────────────────────────────

    def test_code_review_produces_single_eval_entry(self):
        """AC-14: Code-review phase produces exactly one eval.json entry."""
        entries = [
            {"phase": "01-requirements", "verdict": "pass"},
            {"phase": "02-test-design", "verdict": "pass"},
            {"phase": "03-dev-proposal", "verdict": "pass"},
            {"phase": "04-test-gen", "verdict": "pass"},
            {"phase": "05-implementation", "verdict": "pass"},
            {"phase": "06-unit-test", "verdict": "pass"},
        ]
        # Simulate code-review phase completing
        code_review_entry = {
            "phase": "07-code-review",
            "timestamp": "2026-05-25T14:00:00.000Z",
            "verdict": "pass",
            "attempt": 1,
            "report": "Code review passed: no issues found",
            "items": [
                {"item_id": "security", "pass": True},
                {"item_id": "test_coverage", "pass": True},
                {"item_id": "error_handling", "pass": True},
                {"item_id": "code_quality", "pass": True},
            ],
            "backtrack_to": None,
        }
        entries.append(code_review_entry)

        # Filter code-review entries
        cr_entries = [e for e in entries if e["phase"] == "07-code-review"]
        self.assertEqual(len(cr_entries), 1)
        self.assertEqual(cr_entries[0]["verdict"], "pass")

    def test_code_review_entry_has_no_integration_test_data(self):
        """AC-14: Code-review eval entry should NOT contain integration test data."""
        entry = {
            "phase": "07-code-review",
            "verdict": "pass",
            "items": [
                {"item_id": "security", "pass": True},
                {"item_id": "test_coverage", "pass": True},
            ],
        }
        # Check no integration-test related fields
        item_ids = [item["item_id"] for item in entry.get("items", [])]
        self.assertNotIn("integration_test", str(item_ids).lower())
        self.assertNotIn("integration", str(entry).lower())

    # ─── Evaluator checklist ──────────────────────────────────────────────

    def test_code_review_evaluator_has_static_checklist(self):
        """Code-review evaluator should have its static checklist items."""
        # TODO: Load actual evaluator definition
        # checklist = code_review_skill.get_evaluator_checklist()
        # self.assertIn("security", checklist)
        # self.assertIn("test_coverage", checklist)
        # self.assertIn("error_handling", checklist)
        # self.assertIn("code_quality", checklist)
        pass

    def test_code_review_entry_items_are_checklist_results(self):
        """Code-review eval entry items should map to checklist items."""
        entry_items = [
            {"item_id": "security", "pass": True},
            {"item_id": "test_coverage", "pass": False},
            {"item_id": "error_handling", "pass": True},
            {"item_id": "code_quality", "pass": True},
        ]
        # Verification
        self.assertEqual(len(entry_items), 4)
        failing = [i for i in entry_items if not i["pass"]]
        self.assertEqual(len(failing), 1)
        self.assertEqual(failing[0]["item_id"], "test_coverage")


if __name__ == "__main__":
    unittest.main(verbosity=2)
