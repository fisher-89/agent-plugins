#!/usr/bin/env python3
"""
Settings configuration verification tests for .claude/settings.local.json.

Verifies that the permissions allowlist in settings.local.json uses
the updated MCP tool names (phase_check, phase_log) and has no
remaining old eval/* entries.

Coverage:
  AC-6: settings.local.json uses new names (phase_check, phase_log)
  AC-11: No eval/* prefix entries remain
"""

import json
import os
import unittest


# ─── Constants ────────────────────────────────────────────────────────────────

PROJECT_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "..", "..", "..", "..")
)

SETTINGS_PATH = os.path.join(PROJECT_ROOT, ".claude", "settings.local.json")

EXPECTED_FQN_PREFIX = "mcp__plugin_dev-team_dev-team__"

EXPECTED_ALLOW_ENTRIES = [
    f"{EXPECTED_FQN_PREFIX}phase_check",
    f"{EXPECTED_FQN_PREFIX}phase_log",
]

# Old entries that should have been cleaned up
OLD_EVAL_ENTRIES = [
    f"{EXPECTED_FQN_PREFIX}eval/check",
    f"{EXPECTED_FQN_PREFIX}eval/log",
]

# Any entry with "eval/" in the FQN that should not remain
OLD_EVAL_PATTERNS = [
    f"{EXPECTED_FQN_PREFIX}eval/",
]


def load_settings():
    """Load and return the parsed settings.local.json content."""
    if not os.path.isfile(SETTINGS_PATH):
        raise FileNotFoundError(f"settings.local.json not found at: {SETTINGS_PATH}")
    with open(SETTINGS_PATH, "r", encoding="utf-8") as f:
        return json.load(f)


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite: Settings Configuration
# ═══════════════════════════════════════════════════════════════════════════════

class TestSettingsConfig(unittest.TestCase):
    """Verify .claude/settings.local.json has updated MCP tool permissions."""

    @classmethod
    def setUpClass(cls):
        try:
            cls.settings = load_settings()
            cls.allow_list = cls.settings.get("permissions", {}).get("allow", [])
        except (FileNotFoundError, json.JSONDecodeError) as e:
            cls.settings = {}
            cls.allow_list = []
            cls._load_error = str(e)

    def assert_settings_loaded(self):
        """Skip test if settings file not found or invalid."""
        if not self.settings:
            self.skipTest(getattr(self, "_load_error", "settings.local.json not loaded"))

    # ─── AC-6: New names correct ──────────────────────────────────────────

    def test_settings_new_names_correct(self):
        """AC-6: Verify settings.local.json uses new phase_check and phase_log names."""
        self.assert_settings_loaded()

        # Assert that the allow list contains phase_check and phase_log
        # entries with the correct FQN format.
        for entry in EXPECTED_ALLOW_ENTRIES:
            self.assertIn(
                entry, self.allow_list,
                f"Expected allow entry '{entry}' not found in settings.local.json"
            )

    # ─── AC-11: No eval prefix remaining ──────────────────────────────────

    def test_no_eval_prefix_remaining(self):
        """AC-11: Verify no old eval/* prefix entries remain in allowlist.

        This covers the special migration case where 'eval/check' -> 'phase_check'
        and 'eval/log' -> 'phase_log', ensuring no vestigial eval/* entries.
        """
        self.assert_settings_loaded()

        # Assert that old eval/ FQN entries are NOT present in the allow list.
        for old_entry in OLD_EVAL_ENTRIES:
            self.assertNotIn(
                old_entry, self.allow_list,
                f"Old eval entry '{old_entry}' still present in settings.local.json"
            )

        # Assert that no entry in the allow list contains 'eval/' as
        # part of the MCP FQN.
        for allow_entry in self.allow_list:
            if allow_entry.startswith(EXPECTED_FQN_PREFIX):
                self.assertNotIn(
                    "eval/", allow_entry,
                    f"Allow entry '{allow_entry}' still uses eval/ prefix"
                )

    # ─── Additional validation ────────────────────────────────────────────

    def test_allow_list_is_array(self):
        """Verify the permissions.allow field is a list."""
        self.assert_settings_loaded()

        # Assert that permissions.allow is a list type.
        self.assertIsInstance(
            self.allow_list, list,
            "permissions.allow should be a list"
        )

    def test_mcp_permissions_use_underscore_only(self):
        """Verify all MCP FQN permissions in allowlist use underscore format."""
        self.assert_settings_loaded()

        # Assert that every entry starting with the MCP FQN prefix
        # uses only underscores (no slashes) in the tool name portion.
        for entry in self.allow_list:
            if entry.startswith(EXPECTED_FQN_PREFIX):
                tool_part = entry[len(EXPECTED_FQN_PREFIX):]
                self.assertNotIn(
                    "/", tool_part,
                    f"Entry '{entry}' has slash in tool name portion"
                )


if __name__ == "__main__":
    unittest.main(verbosity=2)
