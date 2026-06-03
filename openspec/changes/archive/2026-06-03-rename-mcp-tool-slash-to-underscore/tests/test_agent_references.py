#!/usr/bin/env python3
"""
Static reference verification tests for agent files.

Verifies that all agent agent.md files reference MCP tools using
the underscore xx_yy FQN format, with no remaining slash-format FQNs.

Coverage:
  AC-5: All agents use underscore FQN format
"""

import os
import re
import unittest


# ─── Constants ────────────────────────────────────────────────────────────────

PROJECT_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "..", "..", "..", "..")
)

AGENTS_DIR = os.path.join(PROJECT_ROOT, "plugins", "dev-team", "agents")

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
SLASH_FQN_PREFIX = re.escape(EXPECTED_FQN_PREFIX)

# Broad pattern to catch any FQN with a slash in the tool name part
SLASH_FQN_PATTERN = re.compile(rf"{SLASH_FQN_PREFIX}\w+/\w+")


def discover_agent_files():
    """Return a list of (agent_name, file_path) for all agent.md files."""
    if not os.path.isdir(AGENTS_DIR):
        return []
    results = []
    for fname in sorted(os.listdir(AGENTS_DIR)):
        if fname.endswith(".md"):
            fpath = os.path.join(AGENTS_DIR, fname)
            if os.path.isfile(fpath):
                name = fname.replace(".md", "")
                results.append((name, fpath))
    return results


def read_agent_file(file_path):
    """Read an agent.md file and return its content as a string."""
    with open(file_path, "r", encoding="utf-8") as f:
        return f.read()


def extract_fqns(content):
    """Extract all MCP FQN references from content.

    Returns a list of full FQN strings found.
    """
    pattern = re.compile(rf"{SLASH_FQN_PREFIX}[a-z]+[a-z_/]*[a-z]+")
    return pattern.findall(content)


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite: Agent References
# ═══════════════════════════════════════════════════════════════════════════════

class TestAgentReferences(unittest.TestCase):
    """Verify agent agent.md files use underscore FQN format."""

    @classmethod
    def setUpClass(cls):
        cls.agent_files = discover_agent_files()
        cls.agent_contents = {}
        for agent_name, file_path in cls.agent_files:
            try:
                cls.agent_contents[agent_name] = read_agent_file(file_path)
            except Exception:
                cls.agent_contents[agent_name] = ""

    def assert_agents_discovered(self):
        """Skip test if no agent files found."""
        if not self.agent_files:
            self.skipTest(f"No agent.md files found under: {AGENTS_DIR}")

    # ─── AC-5: All agents use underscore ──────────────────────────────────

    def test_all_agents_use_underscore_fqn(self):
        """AC-5 (forward): Verify all agent files reference underscore FQN format."""
        self.assert_agents_discovered()

        # Assert that every FQN reference in every agent file uses
        # underscore format (xx_yy) rather than slash format (xx/yy).
        for agent_name, content in self.agent_contents.items():
            fqns = extract_fqns(content)
            for fqn in fqns:
                self.assertNotIn(
                    "/", fqn,
                    f"Agent '{agent_name}' has slash in FQN: {fqn}"
                )

    def test_no_agent_contains_slash_fqn(self):
        """AC-5 (reverse): Verify no agent file contains old slash-format FQN."""
        self.assert_agents_discovered()

        # Assert that no agent file matches the old FQN pattern
        # mcp__plugin_dev-team_dev-team__<domain>/<operation>.
        for agent_name, content in self.agent_contents.items():
            matches = SLASH_FQN_PATTERN.findall(content)
            self.assertEqual(
                len(matches), 0,
                f"Agent '{agent_name}' contains {len(matches)} slash-format FQN(s): {matches}"
            )

    def test_architecture_agent_specific_tools(self):
        """Verify the architecture agent has all expected archi_* tools.

        The architecture agent is unique in that it references multiple
        archi tools (archi_query, archi_validate, archi_write, archi_check)
        plus phase_log.
        """
        self.assert_agents_discovered()

        # Check the architecture.md agent file specifically for:
        #   - archi_query
        #   - archi_validate
        #   - archi_write
        #   - archi_check
        #   - phase_log
        # Assert that all of these appear in underscore format.
        # TODO: Uncomment and implement specific assertions:
        # archi_agent = self.agent_contents.get("architecture", "")
        # for tool in ["archi_query", "archi_validate", "archi_write", "archi_check", "phase_log"]:
        #     self.assertIn(
        #         f"{EXPECTED_FQN_PREFIX}{tool}", archi_agent,
        #         f"architecture agent missing {tool}"
        #     )
        pass  # TODO: Implement architecture agent tool check

    def test_evaluator_agents_use_phase_log(self):
        """Verify all evaluator agents reference phase_log."""
        self.assert_agents_discovered()

        # Identify evaluator agents (their names end in '-evaluator')
        # and verify that all of them reference the underscore-format
        # phase_log (not phase/log).
        for agent_name, content in self.agent_contents.items():
            if "evaluator" in agent_name:
                self.assertIn(
                    f"{EXPECTED_FQN_PREFIX}phase_log", content,
                    f"Evaluator agent '{agent_name}' missing phase_log FQN"
                )


if __name__ == "__main__":
    unittest.main(verbosity=2)
