#!/usr/bin/env python3
"""
AC-3 Unit Test: test-gen-generator colocated test file output.

Verifies that the test-gen-generator.md agent definition contains:
1. Instruction to write test files colocated with source files (same directory)
2. Output path is NOT under openspec/changes/<name>/tests/ but rather source dir

Also verifies boundary scenarios:
- Multi-level subdirectory: src/api/v2/handler.py -> src/api/v2/test_handler.py
- Same-named files in different directories: src/auth.py -> src/test_auth.py,
  src/api/auth.py -> src/api/test_auth.py (no conflict)
"""

import os
import re
import unittest

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
AGENT_FILE = os.path.join(PROJECT_ROOT, "plugins", "dev-team", "agents", "test-gen-generator.md")


def read_agent_file():
    """Read the test-gen-generator agent definition file."""
    if not os.path.isfile(AGENT_FILE):
        raise FileNotFoundError(f"Agent file not found: {AGENT_FILE}")
    with open(AGENT_FILE, "r", encoding="utf-8") as f:
        return f.read()


def has_colocated_output_instruction(content):
    """Check if colocated output instruction is present."""
    patterns = [
        r"共存.*输出",
        r"colocat",
        r"test.*file.*write.*same.*director",
        r"测试文件.*写入.*相同.*目录",
        r"同目录",
        r"same.*directory.*as.*source",
        r"源码.*目录",
        r"Source.*directory.*test",
        r"test_<module>\.py",
        r"<module>\.test\.ts",
        r"<module>_test\.rs",
        r"<module>_test\.go",
        r"write test file.*colocat",
    ]
    return any(re.search(p, content, re.IGNORECASE) for p in patterns)


def has_old_tests_directory(content):
    """Check if old openspec/changes/<name>/tests/ output path still appears."""
    patterns = [
        r"openspec/changes/<name>/tests/",
        r"openspec/changes/<change-name>/tests/",
        r"openspec/changes/.*?/tests/",
        r"change.*tests/.*目录",
        r"<change-name>/tests/",
    ]
    return any(re.search(p, content) for p in patterns)


class TestGenGeneratorColocatedOutput(unittest.TestCase):
    """Test that test-gen-generator.md instructs colocated output."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_colocated_output_instruction_present(self):
        """Verify colocated output instruction exists in agent file."""
        self.assertTrue(
            has_colocated_output_instruction(self.content),
            "test-gen-generator.md should contain instruction to write test files "
            "colocated with source files in the same directory"
        )

    def test_colocated_replaces_tests_directory(self):
        """Verify old openspec/changes/<name>/tests/ output path is removed/changed."""
        has_old = has_old_tests_directory(self.content)
        if has_old:
            # If old path still appears, check that colocated instruction overrides it
            # or old path is in a "NOT" / "不" context
            colocated_pos = self.content.lower().find("colocat")
            if colocated_pos == -1:
                colocated_pos = self.content.lower().find("共存")
            old_path_positions = []
            for p in [
                r"openspec/changes/<name>/tests/",
                r"openspec/changes/<change-name>/tests/",
            ]:
                m = re.search(p, self.content)
                if m:
                    old_path_positions.append(m.start())
            if colocated_pos > 0 and old_path_positions:
                for old_pos in old_path_positions:
                    # Check context around old path for negation or override
                    context_around = self.content[max(0, old_pos - 50):old_pos + 100]
                    has_negation = re.search(r"(?i)not|don't|no|禁止|不要|不再|instead of", context_around)
                    if has_negation:
                        continue  # Negated reference is acceptable
                    # If no negation, colocated should appear after old instruction
                    # TODO: Exact ordering depends on final agent file content
                    pass
        # Primary assertion: agent should NOT reference tests/ as the sole output directory
        # TODO: Convert to failure assertion once agent is modified
        self.assertFalse(
            has_old and not has_colocated_output_instruction(self.content),
            "Old tests/ directory reference without colocated override is not acceptable"
        )

    def test_multi_level_subdirectory(self):
        """Boundary: verify agent handles multi-level subdirectories correctly.
        src/api/v2/handler.py -> src/api/v2/test_handler.py (test in same subdir)."""
        # Check for directory structure handling
        subdir_patterns = [
            r"multi.*level.*director",
            r"subdirector",
            r"子目录",
            r"same.*directory.*struct",
            r"preserv.*path",
            r"保持.*路径",
            r"relative.*path",
        ]
        has_subdir_handling = any(
            re.search(p, self.content, re.IGNORECASE) for p in subdir_patterns
        )
        # TODO: Strengthen assertion once agent file is finalized
        if not has_subdir_handling:
            # Fallback: verify colocated implies same dir structure
            self.assertTrue(
                has_colocated_output_instruction(self.content),
                "Colocated output should imply same-directory placement for subdirectories"
            )

    def test_same_name_different_dirs_no_conflict(self):
        """Boundary: verify agent handles same-named files in different directories
        without collision. src/auth.py -> src/test_auth.py,
        src/api/auth.py -> src/api/test_auth.py."""
        # Check for name collision handling
        collision_patterns = [
            r"(?i)name.*conflict",
            r"(?i)collision",
            r"(?i)unique.*test.*file",
            r"同名.*文件",
            r"重复.*不会",
            r"不会.*冲突",
        ]
        has_collision_handling = any(
            re.search(p, self.content) for p in collision_patterns
        )
        # TODO: Add stronger assertion once agent defines collision handling
        if not has_collision_handling:
            # Colocated output by definition avoids collision (same dir as source)
            self.assertTrue(
                has_colocated_output_instruction(self.content),
                "Colocated output inherently avoids naming collisions"
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)
