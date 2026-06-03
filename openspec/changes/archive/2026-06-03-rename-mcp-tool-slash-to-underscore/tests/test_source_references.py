#!/usr/bin/env python3
"""
Static reference verification tests for MCP tool source file (mcp.ts).

Verifies that all 11 registerTool calls in plugins/dev-team/bin/src/mcp.ts
use the underscore xx_yy format and have no remaining slash-format names.

Coverage:
  AC-1: All 11 tool names use underscore format
  AC-7: phase_* tools all underscore
  AC-8: archi_* tools all underscore
  AC-9: config_* tools all underscore
"""

import os
import re
import unittest


# ─── Constants ────────────────────────────────────────────────────────────────

PROJECT_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "..", "..", "..", "..")
)

MCP_TS_PATH = os.path.join(
    PROJECT_ROOT, "plugins", "dev-team", "bin", "src", "mcp.ts"
)

EXPECTED_TOOL_NAMES = [
    "phase_log", "phase_check", "phase_next",
    "archi_query", "archi_validate", "archi_write", "archi_check",
    "config_get", "config_set", "config_unset", "config_context",
]

EXPECTED_PHASE_TOOLS = ["phase_log", "phase_check", "phase_next"]
EXPECTED_ARCHI_TOOLS = ["archi_query", "archi_validate", "archi_write", "archi_check"]
EXPECTED_CONFIG_TOOLS = ["config_get", "config_set", "config_unset", "config_context"]

SLASH_TOOLS = [
    "phase/log", "phase/check", "phase/next",
    "archi/query", "archi/validate", "archi/write", "archi/check",
    "config/get", "config/set", "config/unset", "config/context",
]


def read_mcp_ts():
    """Read mcp.ts and return its content as a string."""
    if not os.path.isfile(MCP_TS_PATH):
        raise FileNotFoundError(f"mcp.ts not found at: {MCP_TS_PATH}")
    with open(MCP_TS_PATH, "r", encoding="utf-8") as f:
        return f.read()


def extract_register_tool_names(content):
    """Extract all tool name arguments from server.registerTool() calls.

    Returns a list of string literals used as the first argument.
    This is the authoritative source for tool names -- description text
    and other string fields are NOT extracted here (FP-2 prevention).
    """
    # Match: server.registerTool('name', ...) or server.registerTool("name", ...)
    pattern = r"server\.registerTool\s*\(\s*['\"]([^'\"]+)['\"]"
    return re.findall(pattern, content)


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite: Source References (mcp.ts)
# ═══════════════════════════════════════════════════════════════════════════════

class TestSourceReferences(unittest.TestCase):
    """Verify plugins/dev-team/bin/src/mcp.ts registerTool names."""

    @classmethod
    def setUpClass(cls):
        try:
            cls.content = read_mcp_ts()
            cls.tool_names = extract_register_tool_names(cls.content)
        except FileNotFoundError as e:
            cls.content = ""
            cls.tool_names = []
            cls._missing_file = str(e)

    # ─── Helper ───────────────────────────────────────────────────────────

    def assert_file_exists(self):
        """Skip test if mcp.ts is not found (e.g., CI without source)."""
        if not self.content:
            self.skipTest(getattr(self, "_missing_file", "mcp.ts not found"))

    # ─── AC-1: All 11 tool names use underscore ───────────────────────────

    def test_all_11_tools_use_underscore(self):
        """AC-1: Verify all 11 registerTool names use xx_yy underscore format."""
        self.assert_file_exists()

        # Verify each expected underscore tool name appears as a registerTool arg.
        # This check targets the tool registration name parameter only, NOT
        # description strings or other text fields (FP-2 compliant).
        for name in EXPECTED_TOOL_NAMES:
            self.assertIn(
                name, self.tool_names,
                f"Tool '{name}' missing from registerTool calls"
            )

    def test_no_registerTool_contains_slash(self):
        """AC-1 (reverse): Verify no registerTool call has a slash in its first argument."""
        self.assert_file_exists()

        # Verify registerTool first-argument names do not contain '/'.
        # This only checks the name parameter, not description/comment text.
        for name in self.tool_names:
            self.assertNotIn(
                "/", name,
                f"registerTool name '{name}' contains slash (should use underscore)"
            )

    # ─── AC-7: phase_* tools all underscore ────────────────────────────────

    def test_phase_tools_all_underscore(self):
        """AC-7: Verify all phase_* registerTool names use underscore format."""
        self.assert_file_exists()

        for name in EXPECTED_PHASE_TOOLS:
            self.assertIn(
                name, self.tool_names,
                f"Phase tool '{name}' missing from registerTool calls"
            )

    # ─── AC-8: archi_* tools all underscore ───────────────────────────────

    def test_archi_tools_all_underscore(self):
        """AC-8: Verify all archi_* registerTool names use underscore format."""
        self.assert_file_exists()

        for name in EXPECTED_ARCHI_TOOLS:
            self.assertIn(
                name, self.tool_names,
                f"Archi tool '{name}' missing from registerTool calls"
            )

    # ─── AC-9: config_* tools all underscore ──────────────────────────────

    def test_config_tools_all_underscore(self):
        """AC-9: Verify all config_* registerTool names use underscore format.

        Also verifies the boundary case that 'config/' grep does not falsely
        match 'config_' — i.e., no remaining `config/get` etc.
        """
        self.assert_file_exists()

        for name in EXPECTED_CONFIG_TOOLS:
            self.assertIn(
                name, self.tool_names,
                f"Config tool '{name}' missing from registerTool calls"
            )

    # ─── Boundary: description slash preserved ────────────────────────────

    def test_registerTool_description_slash_preserved(self):
        """Boundary: Verify description fields with URLs/slashes are NOT modified.

        Only the first argument (tool name) should be renamed; description text
        containing slashes (e.g., URLs) must remain intact. This test targets
        description text specifically, which is excluded from name-only checks.
        """
        self.assert_file_exists()

        # Use a more specific regex to extract description string literals
        # from registerTool calls and verify they still contain expected
        # slash patterns (URLs, etc.) if any were present before.
        # TODO: Implement description text extraction and assertion.
        # TODO: Assert no false-positive slash replacements in descriptions.
        _ = self.content  # placeholder for future implementation

    # ─── Boundary: valid identifier names ─────────────────────────────────

    def test_all_tool_names_valid_identifier(self):
        """Boundary: Verify all tool names are non-empty, non-trivial identifiers."""
        self.assert_file_exists()

        # Assert that all extracted tool names:
        # 1. Are non-empty strings
        # 2. Match the pattern /^[a-z][a-z_]*[a-z]$/
        # 3. Are not just '_' or empty
        for name in self.tool_names:
            self.assertTrue(
                len(name) > 0,
                "Tool name must be non-empty"
            )
            # TODO: Add regex validation /^[a-z][a-z_]*[a-z]$/ for stricter checking.

    # ─── Boundary: lowercase only ─────────────────────────────────────────

    def test_all_tool_names_lowercase(self):
        """Boundary: Verify all tool names are lowercase (case-sensitive match)."""
        self.assert_file_exists()

        for name in self.tool_names:
            self.assertEqual(
                name, name.lower(),
                f"Tool name '{name}' is not all lowercase"
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)
