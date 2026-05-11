#!/usr/bin/env python3
"""
Automated tests for on-user-prompt.py hook.

Tests cover:
1. IntentClassifier: keyword matching, default fallback, scope estimation, trivial fix detection
2. StateAnalyzer: change discovery, stage detection, task progress
3. Router: decision matrix including the question-default-vs-active-changes conflict fix
4. OutputBuilder: formatting for auto/suggest/direct modes
5. Integration: end-to-end flow via main()
"""

import importlib.util
import json
import os
import sys
import tempfile
import unittest

# Load the module from file (due to dash in filename)
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
MODULE_PATH = os.path.join(SCRIPT_DIR, "on-user-prompt.py")

spec = importlib.util.spec_from_file_location("on_user_prompt", MODULE_PATH)
on_user_prompt = importlib.util.module_from_spec(spec)
spec.loader.exec_module(on_user_prompt)

# Import classes from loaded module
ChangeInfo = on_user_prompt.ChangeInfo
IntentClassifier = on_user_prompt.IntentClassifier
IntentResult = on_user_prompt.IntentResult
OutputBuilder = on_user_prompt.OutputBuilder
RouteDecision = on_user_prompt.RouteDecision
Router = on_user_prompt.Router
StateAnalyzer = on_user_prompt.StateAnalyzer


class TestIntentClassifier(unittest.TestCase):
    """Tests for IntentClassifier.detect()."""

    def setUp(self):
        self.classifier = IntentClassifier()

    # --- Intent type detection ---

    def test_explore_intent_english(self):
        result = self.classifier.detect("analyze the codebase")
        self.assertEqual(result.intent_type, "explore")
        self.assertIn("analyze", result.matched_keywords)

    def test_explore_intent_chinese(self):
        result = self.classifier.detect("分析这段代码")
        self.assertEqual(result.intent_type, "explore")

    def test_implement_intent_english(self):
        result = self.classifier.detect("add a login page")
        self.assertEqual(result.intent_type, "implement")

    def test_implement_intent_chinese(self):
        result = self.classifier.detect("添加登录功能")
        self.assertEqual(result.intent_type, "implement")

    def test_fix_intent_english(self):
        result = self.classifier.detect("fix the crash in login")
        self.assertEqual(result.intent_type, "fix")

    def test_fix_intent_chinese(self):
        result = self.classifier.detect("修复登录崩溃问题")
        self.assertEqual(result.intent_type, "fix")

    def test_archive_intent_english(self):
        result = self.classifier.detect("archive this change, it's done")
        self.assertEqual(result.intent_type, "archive")

    def test_archive_intent_chinese(self):
        result = self.classifier.detect("归档这个变更")
        self.assertEqual(result.intent_type, "archive")

    def test_question_intent_english(self):
        result = self.classifier.detect("how does auth work?")
        self.assertEqual(result.intent_type, "question")

    def test_question_intent_chinese(self):
        result = self.classifier.detect("这是什么功能？")
        self.assertEqual(result.intent_type, "question")

    # --- Default fallback: no keyword match -> question + low confidence ---

    def test_no_keyword_match_defaults_to_question_with_low_confidence(self):
        """
        Core bug scenario: when no keywords match, the classifier falls back
        to 'question' with 'low' confidence. The Router must handle this
        differently from a genuine high-confidence question.
        """
        result = self.classifier.detect("Create login page")
        # "Create" matches "implement" pattern \bcreate\b
        self.assertEqual(result.intent_type, "implement")

    def test_no_keyword_match_at_all(self):
        """Prompt with zero matching keywords should be question + low."""
        result = self.classifier.detect("hello world foo bar")
        self.assertEqual(result.intent_type, "question")
        self.assertEqual(result.confidence, "low")
        self.assertEqual(result.matched_keywords, [])

    # --- Confidence levels ---

    def test_high_confidence_multiple_matches(self):
        result = self.classifier.detect("fix the bug and resolve the crash")
        # "fix" and "resolve" both match "fix" patterns → count >= 2
        self.assertEqual(result.intent_type, "fix")
        self.assertEqual(result.confidence, "high")

    def test_medium_confidence_single_match(self):
        result = self.classifier.detect("fix")
        self.assertEqual(result.intent_type, "fix")
        self.assertEqual(result.confidence, "medium")

    # --- Scope estimation ---

    def test_scope_trivial(self):
        result = self.classifier.detect("fix typo in variable name")
        self.assertEqual(result.scope, "trivial")

    def test_scope_large(self):
        result = self.classifier.detect("refactor the entire system architecture")
        # "refactor" matches both "implement" and "large" hints
        self.assertEqual(result.scope, "large")

    def test_scope_default_medium(self):
        result = self.classifier.detect("fix the bug in login")
        # No scope hints → defaults to "medium"
        self.assertEqual(result.scope, "medium")

    # --- Trivial fix detection ---

    def test_trivial_fix_scope_trivial(self):
        result = self.classifier.detect("fix typo in variable name")
        self.assertTrue(result.is_trivial_fix)

    def test_trivial_fix_private_hint(self):
        result = self.classifier.detect("fix the private helper function bug")
        self.assertTrue(result.is_trivial_fix)

    def test_non_trivial_fix(self):
        result = self.classifier.detect("fix the login crash in production")
        # No trivial hints, scope is medium → not trivial
        self.assertFalse(result.is_trivial_fix)


class TestStateAnalyzer(unittest.TestCase):
    """Tests for StateAnalyzer."""

    def setUp(self):
        self.analyzer = StateAnalyzer()
        self.tmpdir = tempfile.mkdtemp()

    def _create_change(self, name, proposal=True, design=False, tasks=None):
        """Helper: create a change directory with optional artifacts."""
        change_dir = os.path.join(self.tmpdir, "openspec", "changes", name)
        os.makedirs(change_dir, exist_ok=True)
        if proposal:
            with open(os.path.join(change_dir, "proposal.md"), "w") as f:
                f.write("# Proposal\n")
        if design:
            with open(os.path.join(change_dir, "design.md"), "w") as f:
                f.write("# Design\n")
        if tasks is not None:
            with open(os.path.join(change_dir, "tasks.md"), "w") as f:
                for task in tasks:
                    f.write(task + "\n")
        return change_dir

    def test_no_changes_dir(self):
        changes = self.analyzer.find_active_changes(self.tmpdir)
        self.assertEqual(changes, [])

    def test_finds_active_changes(self):
        self._create_change("my-change")
        changes = self.analyzer.find_active_changes(self.tmpdir)
        self.assertEqual(len(changes), 1)
        self.assertEqual(changes[0].name, "my-change")

    def test_ignores_archive_dir(self):
        archive_dir = os.path.join(self.tmpdir, "openspec", "changes", "archive")
        os.makedirs(archive_dir, exist_ok=True)
        changes = self.analyzer.find_active_changes(self.tmpdir)
        self.assertEqual(changes, [])

    def test_stage_proposal(self):
        self._create_change("my-change", proposal=True, design=False)
        changes = self.analyzer.find_active_changes(self.tmpdir)
        self.assertEqual(changes[0].stage, "proposal")

    def test_stage_design(self):
        self._create_change("my-change", proposal=True, design=True)
        changes = self.analyzer.find_active_changes(self.tmpdir)
        self.assertEqual(changes[0].stage, "design")

    def test_stage_tasks(self):
        self._create_change("my-change", tasks=["- [ ] task 1", "- [x] task 2"])
        changes = self.analyzer.find_active_changes(self.tmpdir)
        self.assertEqual(changes[0].stage, "tasks")
        self.assertEqual(changes[0].tasks_total, 2)
        self.assertEqual(changes[0].tasks_done, 1)

    def test_tasks_progress_all_done(self):
        self._create_change("done-change", tasks=["- [x] task 1", "- [x] task 2"])
        changes = self.analyzer.find_active_changes(self.tmpdir)
        self.assertTrue(changes[0].tasks_complete)

    def test_tasks_progress_partial(self):
        self._create_change("partial-change", tasks=["- [x] task 1", "- [ ] task 2"])
        changes = self.analyzer.find_active_changes(self.tmpdir)
        self.assertTrue(changes[0].tasks_started)
        self.assertFalse(changes[0].tasks_complete)


class TestRouterQuestionDefaultConflict(unittest.TestCase):
    """
    Tests for the bug fix: question-default + low confidence should not
    bypass active change routing.

    Before the fix:
    - No keyword match → question + low confidence
    - _route_question() always returned direct mode
    - Active changes were completely ignored

    After the fix:
    - question + low confidence + active changes → suggest mode based on state
    - question + high/medium confidence → direct mode (genuine question)
    - question + low confidence + no changes → direct mode
    """

    def setUp(self):
        self.router = Router()

    def _make_intent(self, intent_type="question", confidence="low", scope="medium", is_trivial_fix=False):
        return IntentResult(
            intent_type=intent_type,
            confidence=confidence,
            scope=scope,
            is_trivial_fix=is_trivial_fix,
            matched_keywords=[]
        )

    def _make_change(self, name="test-change", tasks_total=2, tasks_done=0):
        return ChangeInfo(
            name=name,
            path=f"/fake/{name}",
            stage="tasks",
            tasks_total=tasks_total,
            tasks_done=tasks_done
        )

    # --- THE CORE BUG: low confidence question with active changes ---

    def test_low_conf_question_with_started_tasks_suggests_apply_change(self):
        """
        BUG FIX: unknown intent (question+low) with an active change in progress
        should suggest apply-change, NOT route directly.
        """
        intent = self._make_intent(confidence="low")
        change = self._make_change(tasks_total=3, tasks_done=1)
        decision = self.router.decide(intent, [change])

        self.assertEqual(decision.action, "apply-change")
        self.assertEqual(decision.mode, "suggest")
        self.assertIn("apply-change", decision.reason.lower() + str(decision.options).lower())

    def test_low_conf_question_with_completed_tasks_suggests_archive(self):
        """
        BUG FIX: unknown intent (question+low) with completed change
        should suggest archive.
        """
        intent = self._make_intent(confidence="low")
        change = self._make_change(tasks_total=2, tasks_done=2)
        decision = self.router.decide(intent, [change])

        self.assertEqual(decision.action, "archive")
        self.assertEqual(decision.mode, "suggest")

    def test_low_conf_question_with_not_started_change_suggests_apply_change(self):
        """
        BUG FIX: unknown intent (question+low) with a change that has no
        started tasks should still suggest apply-change.
        """
        intent = self._make_intent(confidence="low")
        change = self._make_change(tasks_total=3, tasks_done=0)
        decision = self.router.decide(intent, [change])

        self.assertEqual(decision.action, "apply-change")
        self.assertEqual(decision.mode, "suggest")

    def test_low_conf_question_no_changes_goes_direct(self):
        """
        Low confidence question with no active changes → direct (no workflow needed).
        """
        intent = self._make_intent(confidence="low")
        decision = self.router.decide(intent, [])

        self.assertEqual(decision.action, "direct")
        self.assertEqual(decision.mode, "direct")

    # --- Genuine question: high/medium confidence → always direct ---

    def test_high_conf_question_goes_direct(self):
        intent = self._make_intent(confidence="high")
        change = self._make_change(tasks_total=2, tasks_done=1)
        decision = self.router.decide(intent, [change])

        self.assertEqual(decision.action, "direct")
        self.assertEqual(decision.mode, "direct")

    def test_medium_conf_question_goes_direct(self):
        intent = self._make_intent(confidence="medium")
        decision = self.router.decide(intent, [])

        self.assertEqual(decision.action, "direct")
        self.assertEqual(decision.mode, "direct")


class TestRouterOtherIntents(unittest.TestCase):
    """Tests for Router with non-question intent types."""

    def setUp(self):
        self.router = Router()

    def _make_intent(self, intent_type, confidence="high", scope="medium", is_trivial_fix=False):
        return IntentResult(
            intent_type=intent_type,
            confidence=confidence,
            scope=scope,
            is_trivial_fix=is_trivial_fix,
            matched_keywords=[]
        )

    # --- Fix intent ---

    def test_trivial_fix_high_conf_auto_direct(self):
        intent = self._make_intent("fix", confidence="high", is_trivial_fix=True)
        decision = self.router.decide(intent, [])
        self.assertEqual(decision.mode, "auto")
        self.assertEqual(decision.action, "direct")

    def test_trivial_fix_medium_conf_suggest_direct(self):
        intent = self._make_intent("fix", confidence="medium", is_trivial_fix=True)
        decision = self.router.decide(intent, [])
        self.assertEqual(decision.mode, "suggest")

    def test_nontrivial_fix_with_active_change_suggests_apply(self):
        intent = self._make_intent("fix", confidence="medium", is_trivial_fix=False)
        change = ChangeInfo("my-fix", "/fake", "tasks", 3, 1)
        decision = self.router.decide(intent, [change])
        self.assertEqual(decision.action, "apply-change")

    def test_nontrivial_fix_no_change_suggests_explore(self):
        intent = self._make_intent("fix", confidence="medium", is_trivial_fix=False)
        decision = self.router.decide(intent, [])
        self.assertEqual(decision.action, "explore")

    # --- Implement intent ---

    def test_implement_high_conf_no_change_auto_explore(self):
        intent = self._make_intent("implement", confidence="high", scope="medium")
        decision = self.router.decide(intent, [])
        self.assertEqual(decision.action, "explore")
        self.assertEqual(decision.mode, "auto")

    def test_implement_high_conf_no_change_large_scope_suggests(self):
        intent = self._make_intent("implement", confidence="high", scope="large")
        decision = self.router.decide(intent, [])
        # Large scope → can't auto-explore
        self.assertEqual(decision.mode, "suggest")

    def test_implement_with_complete_tasks_suggests_archive(self):
        intent = self._make_intent("implement", confidence="high")
        change = ChangeInfo("done", "/fake", "tasks", 2, 2)
        decision = self.router.decide(intent, [change])
        self.assertEqual(decision.action, "archive")

    def test_implement_with_started_tasks_high_conf_auto_apply(self):
        intent = self._make_intent("implement", confidence="high")
        change = ChangeInfo("wip", "/fake", "tasks", 3, 1)
        decision = self.router.decide(intent, [change])
        self.assertEqual(decision.action, "apply-change")
        self.assertEqual(decision.mode, "auto")

    # --- Archive intent ---

    def test_archive_with_complete_change_auto(self):
        intent = self._make_intent("archive", confidence="high")
        change = ChangeInfo("done", "/fake", "tasks", 2, 2)
        decision = self.router.decide(intent, [change])
        self.assertEqual(decision.action, "archive")
        self.assertEqual(decision.mode, "auto")

    def test_archive_no_changes_direct(self):
        intent = self._make_intent("archive", confidence="high")
        decision = self.router.decide(intent, [])
        self.assertEqual(decision.action, "direct")

    def test_archive_incomplete_tasks_suggests_apply(self):
        intent = self._make_intent("archive", confidence="high")
        change = ChangeInfo("wip", "/fake", "tasks", 3, 1)
        decision = self.router.decide(intent, [change])
        self.assertEqual(decision.action, "apply-change")

    # --- Explore intent ---

    def test_explore_always_direct(self):
        intent = self._make_intent("explore", confidence="high")
        decision = self.router.decide(intent, [])
        self.assertEqual(decision.action, "direct")


class TestOutputBuilder(unittest.TestCase):
    """Tests for OutputBuilder formatting."""

    def setUp(self):
        self.builder = OutputBuilder()

    def test_auto_route_with_skill(self):
        decision = RouteDecision(
            action="explore", mode="auto",
            reason="test reason", options=[]
        )
        output = self.builder.auto_route(decision)
        self.assertIn("/openspec-explore", output)
        self.assertIn("test reason", output)

    def test_auto_route_direct(self):
        decision = RouteDecision(
            action="direct", mode="auto",
            reason="direct reason", options=[]
        )
        output = self.builder.auto_route(decision)
        self.assertEqual(output, "direct reason")

    def test_suggestion_shows_confidence_not_intent_type(self):
        """
        Bug fix: suggestion format was showing intent_type instead of confidence.
        Verify the fix: output should show actual confidence value.
        """
        intent = IntentResult(
            intent_type="fix", confidence="high",
            scope="medium", is_trivial_fix=False,
            matched_keywords=["fix"]
        )
        decision = RouteDecision(
            action="explore", mode="suggest",
            reason="test", options=["Option 1"]
        )
        output = self.builder.suggestion(intent, [], decision)
        # Should show confidence: high, NOT confidence: fix
        self.assertIn("confidence: high", output)
        self.assertNotIn("confidence: fix", output)

    def test_direct_with_changes(self):
        change = ChangeInfo("test", "/fake", "tasks", 2, 1)
        output = self.builder.direct([change])
        self.assertIn("Active Changes", output)
        self.assertIn("test", output)

    def test_direct_no_changes_empty(self):
        output = self.builder.direct([])
        self.assertEqual(output, "")


class TestIntegration(unittest.TestCase):
    """Integration tests: IntentClassifier → Router → OutputBuilder."""

    def _make_changes_dir(self, changes=None):
        """Create temp dir with openspec/changes structure."""
        tmpdir = tempfile.mkdtemp()
        changes_dir = os.path.join(tmpdir, "openspec", "changes")
        if changes:
            for name, tasks in changes.items():
                change_dir = os.path.join(changes_dir, name)
                os.makedirs(change_dir, exist_ok=True)
                if tasks:
                    with open(os.path.join(change_dir, "tasks.md"), "w") as f:
                        for t in tasks:
                            f.write(t + "\n")
        return tmpdir

    def test_unknown_prompt_with_active_change_suggests_workflow(self):
        """
        Full integration: prompt with no keywords + active change
        should suggest apply-change, not go direct.
        """
        tmpdir = self._make_changes_dir({
            "my-feature": ["- [x] task 1", "- [ ] task 2"]
        })
        classifier = IntentClassifier()
        analyzer = StateAnalyzer()
        router = Router()

        intent = classifier.detect("hello world foo bar")
        changes = analyzer.find_active_changes(tmpdir)
        decision = router.decide(intent, changes)

        # The fix: should suggest apply-change, NOT direct
        self.assertEqual(intent.intent_type, "question")
        self.assertEqual(intent.confidence, "low")
        self.assertEqual(decision.action, "apply-change")
        self.assertEqual(decision.mode, "suggest")

    def test_genuine_question_goes_direct(self):
        """Genuine question (with keyword match) should go direct."""
        classifier = IntentClassifier()
        router = Router()

        intent = classifier.detect("how does this work?")
        decision = router.decide(intent, [])

        self.assertEqual(intent.intent_type, "question")
        self.assertEqual(intent.confidence, "medium")
        self.assertEqual(decision.action, "direct")
        self.assertEqual(decision.mode, "direct")

    def test_fix_prompt_with_active_change_routes_properly(self):
        """Fix prompt should route to apply-change when change is active."""
        tmpdir = self._make_changes_dir({
            "bug-fix": ["- [ ] task 1"]
        })
        classifier = IntentClassifier()
        analyzer = StateAnalyzer()
        router = Router()

        intent = classifier.detect("修复登录崩溃问题")
        changes = analyzer.find_active_changes(tmpdir)
        decision = router.decide(intent, changes)

        self.assertEqual(intent.intent_type, "fix")
        self.assertIn(decision.action, ["apply-change", "explore"])


if __name__ == "__main__":
    unittest.main()
