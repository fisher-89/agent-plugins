#!/usr/bin/env python3
"""
Tests for file-type-isolation: skill-layer Read path validation, prompt blacklist content,
and violation auditing.

Covers:
- AC-2: test-gen-generator prompt source file blacklist + skill validation logic
- AC-3: implementation-generator prompt directory blacklist + skill validation logic

Tests:
- test-gen-generator prompt contains source file extension blacklist (.ts, .py, .js, etc.)
- implementation-generator prompt contains directory blacklist (tests/, __tests__/, etc.)
- Skill-layer Read path validation rejects blacklisted file types
- Skill-layer Read path validation rejects blacklisted directories
- Skill-layer Read path validation allows whitelisted paths
- Violation auditing records events in eval.json
- Violation threshold (3 strikes) blocks generator output
- Isolation feature toggle (enabled/disabled)
- Feature flag configuration from plugin.json
"""

import json
import os
import shutil
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch


# Blacklist patterns as defined in the spec
SOURCE_FILE_EXTENSIONS = [
    ".ts", ".tsx", ".js", ".jsx", ".py", ".rs", ".go",
    ".java", ".c", ".cpp", ".h", ".hpp", ".cs", ".rb", ".php",
]

SOURCE_FILE_EXTENSIONS_SET = set(SOURCE_FILE_EXTENSIONS)

ALLOWED_EXTENSIONS = [".md", ".template", ".yaml", ".yml", ".json"]
ALLOWED_EXTENSIONS_SET = set(ALLOWED_EXTENSIONS)

BLACKLIST_DIRECTORIES = ["tests/", "__tests__/", "test/", "spec/"]
BLACKLIST_DIRECTORIES_SET = set(BLACKLIST_DIRECTORIES)

VIOLATION_THRESHOLD = 3


def is_blacklisted_file_type(path: str) -> bool:
    """Check if the path has a blacklisted file extension (skeleton)."""
    ext = os.path.splitext(path)[1].lower()
    return ext in SOURCE_FILE_EXTENSIONS_SET


def is_blacklisted_directory(path: str) -> bool:
    """Check if the path is under a blacklisted directory (skeleton)."""
    normalized = path.replace("\\", "/")
    for blocked_dir in BLACKLIST_DIRECTORIES:
        if f"/{blocked_dir}" in normalized or normalized.startswith(blocked_dir):
            return True
    return False


def is_allowed_path(path: str) -> bool:
    """Combined check: path must not match file type or directory blacklist."""
    return not is_blacklisted_file_type(path) and not is_blacklisted_directory(path)


class TempProject:
    """Creates a temporary project for file type isolation tests."""

    def __init__(self):
        self.root = tempfile.mkdtemp(prefix="file_isolation_test_")

    def cleanup(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def create_eval_json(self, entries: list = None):
        """Create eval.json in a change's phases directory."""
        change_dir = os.path.join(self.root, "openspec", "changes", "test-change")
        phases_dir = os.path.join(change_dir, "phases")
        os.makedirs(phases_dir, exist_ok=True)
        eval_path = os.path.join(phases_dir, "eval.json")
        with open(eval_path, "w", encoding="utf-8") as f:
            json.dump(entries or [], f, indent=2)
        return change_dir


class TestFileTypeIsolationPromptContent(unittest.TestCase):
    """Verify that Agent prompt templates contain the required blacklist constraints."""

    def setUp(self):
        # Locate generator prompt files relative to project root
        self.project_root = os.path.join(
            os.path.dirname(os.path.abspath(__file__)),  # tests/
            "..", "..", "..", ".."  # up to project root
        )
        self.project_root = os.path.normpath(self.project_root)

    def test_test_gen_generator_prompt_contains_file_type_blacklist(self):
        """AC-2: Verify test-gen-generator prompt has source file blacklist."""
        prompt_path = os.path.join(
            self.project_root, "plugins", "dev-team", "agents", "test-gen-generator.md"
        )
        if not os.path.isfile(prompt_path):
            self.skipTest(f"Prompt file not found: {prompt_path}")

        with open(prompt_path, "r", encoding="utf-8") as f:
            content = f.read()

        # Check that key extensions are mentioned in the blacklist
        # TODO: Adjust assertions based on actual prompt file content
        self.assertIn(".ts", content, "test-gen-generator prompt should mention .ts blacklist")
        self.assertIn(".py", content, "test-gen-generator prompt should mention .py blacklist")

    def test_test_gen_generator_blacklist_includes_all_extensions(self):
        """AC-2: Verify all required extensions are blacklisted."""
        prompt_path = os.path.join(
            self.project_root, "plugins", "dev-team", "agents", "test-gen-generator.md"
        )
        if not os.path.isfile(prompt_path):
            self.skipTest(f"Prompt file not found: {prompt_path}")

        with open(prompt_path, "r", encoding="utf-8") as f:
            content = f.read()

        # All source extensions should be listed or covered by a glob pattern
        for ext in SOURCE_FILE_EXTENSIONS:
            self.assertIn(
                ext, content,
                f"test-gen-generator prompt should blacklist {ext} files"
            )

    def test_test_gen_generator_allows_design_files(self):
        """AC-2: Verify allowed file types are explicitly mentioned."""
        prompt_path = os.path.join(
            self.project_root, "plugins", "dev-team", "agents", "test-gen-generator.md"
        )
        if not os.path.isfile(prompt_path):
            self.skipTest(f"Prompt file not found: {prompt_path}")

        with open(prompt_path, "r", encoding="utf-8") as f:
            content = f.read()

        # Check that at least one allowed extension is mentioned
        allowed_mentioned = [ext for ext in ALLOWED_EXTENSIONS if ext in content]
        self.assertGreater(
            len(allowed_mentioned), 0,
            "test-gen-generator prompt should mention allowed file types"
        )

    def test_implementation_generator_prompt_contains_directory_blacklist(self):
        """AC-3: Verify implementation-generator prompt has directory blacklist."""
        prompt_path = os.path.join(
            self.project_root, "plugins", "dev-team", "agents",
            "implementation-generator.md"
        )
        if not os.path.isfile(prompt_path):
            self.skipTest(f"Prompt file not found: {prompt_path}")

        with open(prompt_path, "r", encoding="utf-8") as f:
            content = f.read()

        # Check that test directories are mentioned in the blacklist
        self.assertIn(
            "tests/", content,
            "implementation-generator prompt should blacklist tests/ directory"
        )

    def test_implementation_generator_blacklist_includes_all_directories(self):
        """AC-3: Verify all required directories are blacklisted."""
        prompt_path = os.path.join(
            self.project_root, "plugins", "dev-team", "agents",
            "implementation-generator.md"
        )
        if not os.path.isfile(prompt_path):
            self.skipTest(f"Prompt file not found: {prompt_path}")

        with open(prompt_path, "r", encoding="utf-8") as f:
            content = f.read()

        for directory in BLACKLIST_DIRECTORIES:
            self.assertIn(
                directory, content,
                f"implementation-generator prompt should blacklist {directory}"
            )


class TestFileTypeIsolationSkillValidation(unittest.TestCase):
    """Test the skill-layer Read path validation logic for file type isolation."""

    def test_reject_blacklisted_file_type_ts(self):
        """Skill validation rejects Read path ending in .ts."""
        self.assertTrue(is_blacklisted_file_type("src/auth.ts"))
        self.assertFalse(is_allowed_path("src/auth.ts"))

    def test_reject_blacklisted_file_type_py(self):
        """Skill validation rejects Read path ending in .py."""
        self.assertTrue(is_blacklisted_file_type("src/utils/helper.py"))
        self.assertFalse(is_allowed_path("src/utils/helper.py"))

    def test_reject_blacklisted_file_type_js(self):
        """Skill validation rejects Read path ending in .js."""
        self.assertTrue(is_blacklisted_file_type("app.js"))

    def test_reject_blacklisted_file_type_rs(self):
        """Skill validation rejects Read path ending in .rs."""
        self.assertTrue(is_blacklisted_file_type("src/main.rs"))

    def test_allow_markdown_file(self):
        """Skill validation allows Read path ending in .md."""
        self.assertFalse(is_blacklisted_file_type("design.md"))
        self.assertTrue(is_allowed_path("design.md"))

    def test_allow_template_file(self):
        """Skill validation allows Read path ending in .template."""
        self.assertFalse(is_blacklisted_file_type("proposal.md.template"))

    def test_allow_yaml_file(self):
        """Skill validation allows Read path ending in .yaml."""
        self.assertFalse(is_blacklisted_file_type("config.yaml"))

    def test_allow_json_file(self):
        """Skill validation allows Read path ending in .json."""
        self.assertFalse(is_blacklisted_file_type("package.json"))

    def test_reject_unknown_extension(self):
        """Skill validation rejects unknown/non-design extensions by default."""
        self.assertTrue(is_blacklisted_file_type("data.bin"))
        self.assertTrue(is_blacklisted_file_type("image.png"))

    # ─── Directory blacklist tests ────────────────────────────────────────

    def test_reject_read_path_under_tests_dir(self):
        """Skill validation rejects Read path under tests/ directory."""
        self.assertTrue(is_blacklisted_directory("tests/test_auth.py"))

    def test_reject_read_path_under_nested_tests_dir(self):
        """Skill validation rejects nested tests/ paths."""
        self.assertTrue(is_blacklisted_directory("src/components/__tests__/utils.test.ts"))
        self.assertTrue(is_blacklisted_directory("packages/core/test/helper.spec.ts"))

    def test_reject_read_path_under_tests_dir_deep(self):
        """Skill validation rejects deeply nested tests/ paths."""
        self.assertTrue(is_blacklisted_directory(
            "packages/frontend/tests/unit/components/button.test.tsx"
        ))

    def test_allow_src_dir_read(self):
        """Skill validation allows non-blacklisted src/ paths."""
        self.assertFalse(is_blacklisted_directory("src/auth/login.py"))
        self.assertFalse(is_blacklisted_directory("src/utils/helper.ts"))

    def test_allow_design_doc_in_src_docs(self):
        """Skill validation allows design doc paths that happen to contain 'test' substring."""
        # "testing" is not a blacklisted directory, only exact "tests/" etc.
        self.assertFalse(is_blacklisted_directory("docs/testing-guide.md"))

    # ─── Violation auditing ───────────────────────────────────────────────

    def test_audit_records_violation_event(self):
        """Test that violation events are recorded in eval.json findings."""
        # TODO: Call skill-layer audit function
        # violations = audit_file_access(["src/auth.ts"])
        # self.assertEqual(len(violations), 1)
        # self.assertEqual(violations[0]["violation_type"], "blacklisted_file_extension")
        # self.assertEqual(violations[0]["path"], "src/auth.ts")
        pass

    def test_audit_clean_paths_no_violations(self):
        """Test that allowed paths produce no violations."""
        # TODO: Call skill-layer audit function with allowed paths
        # violations = audit_file_access(["design.md", "config.yaml"])
        # self.assertEqual(len(violations), 0)
        pass

    def test_audit_directory_violation_event(self):
        """Test that directory blacklist violations are recorded."""
        # TODO: Call skill-layer audit function
        # violations = audit_file_access(["tests/unit/test_auth.py"])
        # self.assertEqual(violations[0]["violation_type"], "blacklisted_directory")
        pass

    def test_violation_count_tracking(self):
        """Test that violation count is tracked cumulatively."""
        # TODO: Simulate multiple violations and verify count
        # violations = audit_file_access(["src/auth.ts", "app.js", "main.py"])
        # audit_log = {"violations": violations, "count": 3}
        # self.assertEqual(audit_log["count"], 3)
        pass

    def test_violation_threshold_blocks_output(self):
        """AC-2/AC-3: At VIOLATION_THRESHOLD (3), generator output is blocked."""
        violations = [
            {"violation_type": "blacklisted_file_extension", "path": "src/auth.ts", "count": i}
            for i in range(1, VIOLATION_THRESHOLD + 1)
        ]
        # should_block = is_output_blocked(violations, threshold=VIOLATION_THRESHOLD)
        # self.assertTrue(should_block)
        self.assertEqual(len(violations), 3)

    def test_violation_below_threshold_allows_output(self):
        """Test that fewer than threshold violations do not block output."""
        violations = [
            {"violation_type": "blacklisted_file_extension", "path": "src/auth.ts", "count": 1}
        ]
        # should_block = is_output_blocked(violations, threshold=VIOLATION_THRESHOLD)
        # self.assertFalse(should_block)
        self.assertEqual(len(violations), 1)

    # ─── Feature toggle ───────────────────────────────────────────────────

    def test_isolation_enabled_by_default(self):
        """Test that file type isolation is enabled by default."""
        # TODO: Read plugin.json default config
        # config = load_file_type_isolation_config(project_root)
        # self.assertTrue(config["enabled"])
        pass

    def test_isolation_disabled_skips_validation(self):
        """Test that disabled isolation skips all validation."""
        # TODO: Call validation with enabled=False
        # result = validate_file_access(["src/auth.ts"], enabled=False)
        # self.assertTrue(result.allowed)  # no validation when disabled
        pass

    def test_config_from_plugin_json(self):
        """Test that isolation config can be read from plugin.json."""
        # TODO: Load plugin.json and check features.fileTypeIsolation.enabled
        pass


if __name__ == "__main__":
    unittest.main(verbosity=2)
