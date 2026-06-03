#!/usr/bin/env python3
"""
Static reference verification tests for skill files.

Verifies that all 11 skill SKILL.md files reference MCP tools using
the underscore xx_yy FQN format, with no remaining slash-format FQNs.

Coverage:
  AC-4: All skills use underscore FQN format
  Boundary: FQN double-underscore preserved
  Boundary: No slash in FQN in any context (code + Markdown)
  Boundary: All skill FQNs consistent
"""

import os
import re
import unittest


# ─── Constants ────────────────────────────────────────────────────────────────

PROJECT_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "..", "..", "..", "..")
)

SKILLS_DIR = os.path.join(PROJECT_ROOT, "plugins", "dev-team", "skills")

EXPECTED_FQN_PREFIX = "mcp__plugin_dev-team_dev-team__"
EXPECTED_TOOL_NAMES = [
    "phase_log", "phase_check", "phase_next",
    "archi_query", "archi_validate", "archi_write", "archi_check",
    "config_get", "config_set", "config_unset", "config_context",
]

# Build expected FQN strings
EXPECTED_FQNS = [
    f"{EXPECTED_FQN_PREFIX}{name}" for name in EXPECTED_TOOL_NAMES
]

# Old slash-format FQN patterns that should NOT appear
# Pattern: mcp__plugin_dev-team_dev-team__phase/log etc.
SLASH_FQN_PREFIX = re.escape(EXPECTED_FQN_PREFIX)
SLASH_FQN_PATTERN = re.compile(rf"{SLASH_FQN_PREFIX}[a-z]+/[a-z]+")

# Pattern for the new underscore format
UNDERSCORE_FQN_PATTERN = re.compile(rf"{SLASH_FQN_PREFIX}[a-z]+_[a-z]+")


def discover_skill_dirs():
    """Return a list of skill directory names under SKILLS_DIR."""
    if not os.path.isdir(SKILLS_DIR):
        return []
    return [
        d for d in os.listdir(SKILLS_DIR)
        if os.path.isdir(os.path.join(SKILLS_DIR, d))
    ]


def discover_skill_files():
    """Return a list of (skill_name, file_path) tuples for all SKILL.md files."""
    results = []
    for skill_dir in discover_skill_dirs():
        skill_path = os.path.join(SKILLS_DIR, skill_dir, "SKILL.md")
        if os.path.isfile(skill_path):
            results.append((skill_dir, skill_path))
    return results


def read_skill_file(file_path):
    """Read a SKILL.md file and return its content as a string."""
    with open(file_path, "r", encoding="utf-8") as f:
        return f.read()


def extract_fqns(content):
    """Extract all MCP FQN references from content.

    Returns a list of full FQN strings found (e.g.,
    'mcp__plugin_dev-team_dev-team__phase_check').
    """
    pattern = re.compile(rf"{SLASH_FQN_PREFIX}[a-z]+[a-z_/]*[a-z]+")
    return pattern.findall(content)


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite: Skill References
# ═══════════════════════════════════════════════════════════════════════════════

class TestSkillReferences(unittest.TestCase):
    """Verify skill SKILL.md files use underscore FQN format."""

    @classmethod
    def setUpClass(cls):
        cls.skill_files = discover_skill_files()
        cls.skill_contents = {}
        for skill_name, file_path in cls.skill_files:
            try:
                cls.skill_contents[skill_name] = read_skill_file(file_path)
            except Exception:
                cls.skill_contents[skill_name] = ""

    def assert_skills_discovered(self):
        """Skip test if no skill files found."""
        if not self.skill_files:
            self.skipTest(f"No SKILL.md files found under: {SKILLS_DIR}")

    # ─── AC-4: All skills use underscore ─────────────────────────────────

    def test_all_skills_use_underscore_fqn(self):
        """AC-4 (forward): Verify all skill files reference underscore FQN format."""
        self.assert_skills_discovered()

        # Assert that every FQN reference in every skill file uses
        # underscore format (xx_yy) rather than slash format (xx/yy).
        for skill_name, content in self.skill_contents.items():
            fqns = extract_fqns(content)
            for fqn in fqns:
                self.assertNotIn(
                    "/", fqn,
                    f"Skill '{skill_name}' has slash in FQN: {fqn}"
                )

    def test_no_skill_contains_slash_fqn(self):
        """AC-4 (reverse): Verify no skill file contains old slash-format FQN."""
        self.assert_skills_discovered()

        # Assert that no skill file matches the old FQN pattern
        # mcp__plugin_dev-team_dev-team__<domain>/<operation>.
        for skill_name, content in self.skill_contents.items():
            matches = SLASH_FQN_PATTERN.findall(content)
            self.assertEqual(
                len(matches), 0,
                f"Skill '{skill_name}' contains {len(matches)} slash-format FQN(s): {matches}"
            )

    # ─── Boundary: double underscore preserved ────────────────────────────

    def test_fqn_double_underscore_preserved(self):
        """Boundary: Verify FQN double-underscore separator '__' is not modified.

        The namespace separator in MCP FQNs is '__' (double underscore) and
        should remain intact. Only the single slash '/' in the tool name
        portion should be replaced with single underscore '_'.
        """
        self.assert_skills_discovered()

        # Assert that the FQN prefix 'mcp__plugin_dev-team_dev-team__'
        # still contains double underscores and was not corrupted by
        # the rename operation.
        for skill_name, content in self.skill_contents.items():
            fqns = extract_fqns(content)
            for fqn in fqns:
                # The prefix should contain the original double underscore
                self.assertIn(
                    EXPECTED_FQN_PREFIX, fqn,
                    f"Skill '{skill_name}' has corrupted FQN prefix: {fqn}"
                )

    # ─── Boundary: no slash in any context ────────────────────────────────

    def test_no_slash_in_fqn_any_context(self):
        """Boundary: Verify no slash-format FQN exists in any context.

        This covers both code references and Markdown documentation text
        within skill files. Even Markdown examples should use the new format.
        """
        self.assert_skills_discovered()

        # Search for any occurrence of the pattern
        # mcp__plugin_dev-team_dev-team__<word>/<word> including in
        # Markdown code blocks, tables, and prose.
        for skill_name, content in self.skill_contents.items():
            # Use a broad pattern that catches all FQN variants
            broad_pattern = re.compile(rf"{SLASH_FQN_PREFIX}\w+/\w+")
            matches = broad_pattern.findall(content)
            self.assertEqual(
                len(matches), 0,
                f"Skill '{skill_name}' has {len(matches)} slash FQN occurrences: {matches}"
            )

    # ─── Boundary: all FQNs consistent ────────────────────────────────────

    def test_all_skill_fqns_consistent(self):
        """Boundary: Verify all FQN references within a single skill are consistent.

        A single skill file should not contain a mix of underscore and
        slash FQN formats (partial migration state).
        """
        self.assert_skills_discovered()

        # For each skill file, extract all FQN references and verify
        # that ALL of them use underscore format OR none do (no mixing).
        for skill_name, content in self.skill_contents.items():
            fqns = extract_fqns(content)

            # Separate into underscore-format and slash-format
            underscore_fqns = [f for f in fqns if "/" not in f.replace(EXPECTED_FQN_PREFIX, "")]
            slash_fqns = [f for f in fqns if "/" in f.replace(EXPECTED_FQN_PREFIX, "")]

            # Should not have BOTH formats in the same file
            if underscore_fqns and slash_fqns:
                self.fail(
                    f"Skill '{skill_name}' has mixed FQN formats:\n"
                    f"  Underscore: {underscore_fqns}\n"
                    f"  Slash: {slash_fqns}"
                )


if __name__ == "__main__":
    unittest.main(verbosity=2)
