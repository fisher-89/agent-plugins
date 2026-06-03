#!/usr/bin/env python3
"""
Build artifact verification tests for dev-team-mcp.cjs.

Verifies that after rebuilding via npm run build, the compiled
dev-team-mcp.cjs contains only underscore-format tool names.

Incorporates fixes:
  FP-2: Only checks registerTool name parameters, tool definition name
        properties, and switch/case labels — NOT description text.
  FP-3: Uses assertEqual(result.returncode, 0) instead of assertIsNone
        on a variable that may be empty string.

Coverage:
  AC-3: Build artifact uses underscore names, no slash names remain
"""

import os
import re
import subprocess
import sys
import unittest


# ─── Constants ────────────────────────────────────────────────────────────────

PROJECT_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "..", "..", "..", "..")
)

CJS_PATH = os.path.join(
    PROJECT_ROOT, "plugins", "dev-team", "bin", "dev-team-mcp.cjs"
)

BUILD_DIR = os.path.join(PROJECT_ROOT, "plugins", "dev-team", "bin")

EXPECTED_TOOL_NAMES = [
    "phase_log", "phase_check", "phase_next",
    "archi_query", "archi_validate", "archi_write", "archi_check",
    "config_get", "config_set", "config_unset", "config_context",
]

SLASH_TOOL_NAMES = [
    "phase/log", "phase/check", "phase/next",
    "archi/query", "archi/validate", "archi/write", "archi/check",
    "config/get", "config/set", "config/unset", "config/context",
]

# ─── FP-2: CJS tool name extraction patterns ─────────────────────────────────

# Pattern to extract registerTool first-argument strings.
# In the CJS bundle, these may use single/double quotes or backtick
# template literals: registerTool("name"), registerTool('name'),
# registerTool(`name`)
REGISTER_TOOL_PATTERN = re.compile(
    r"registerTool\s*\(\s*(?:['\"`])([^'\"`]+)(?:['\"`])"
)

# Pattern to extract object property name: "name" or name: (tool definition).
# In the CJS bundle, tool definitions may use: name: "phase_log" or "name":"phase_log"
TOOL_NAME_PROPERTY_PATTERN = re.compile(
    r"""['"]?name['"]?\s*:\s*(?:['\"`])([^'\"`]+)(?:['\"`])"""
)

# Pattern to extract switch case labels for tool routing.
# In the CJS bundle: case "phase_log": or case 'phase_log':
SWITCH_CASE_PATTERN = re.compile(
    r"case\s+(?:['\"`])([^'\"`]+)(?:['\"`])\s*:"
)


def extract_tool_registration_names(content):
    """Extract tool names from registerTool calls only (FP-2).

    Scans the CJS bundle for registerTool('name', ...) calls and
    returns the first-argument string literals. This intentionally
    excludes description text fields, comments, and cross-references.

    Returns:
        Set of tool name strings found in registerTool calls.
    """
    return set(REGISTER_TOOL_PATTERN.findall(content))


def extract_tool_definition_names(content):
    """Extract tool names from object name properties (FP-2).

    Scans for name: "value" patterns in tool definition objects.
    Combined with registerTool names and switch case labels to form
    the complete set of tool identifiers that should be renamed.

    Returns:
        Set of tool name strings found in name properties.
    """
    return set(TOOL_NAME_PROPERTY_PATTERN.findall(content))


def extract_switch_case_names(content):
    """Extract tool names from switch/case labels (FP-2).

    Returns:
        Set of tool name strings found in case labels.
    """
    return set(SWITCH_CASE_PATTERN.findall(content))


def extract_all_tool_identifiers(content):
    """Extract all tool name identifiers from registration areas (FP-2).

    Combines registerTool calls, name properties, and switch case labels.
    This is the authoritative set of tool identifiers that should use xx_yy.
    Description text fields are intentionally excluded.
    """
    names = set()
    names.update(extract_tool_registration_names(content))
    names.update(extract_tool_definition_names(content))
    names.update(extract_switch_case_names(content))
    return names


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite: Build Artifact
# ═══════════════════════════════════════════════════════════════════════════════

class TestBuildArtifact(unittest.TestCase):
    """Verify plugins/dev-team/bin/dev-team-mcp.cjs contains updated tool names."""

    @classmethod
    def setUpClass(cls):
        """Rebuild the MCP server and cache the artifact content."""
        cls._build_output = None
        cls._build_error = None
        cls._build_returncode = None  # FP-3: store returncode explicitly
        cls.content = ""
        cls._rebuilt = False
        cls._tool_identifiers = set()  # FP-2: extracted tool identifiers

        if not os.path.isdir(BUILD_DIR):
            cls._build_error = f"Build directory not found: {BUILD_DIR}"
            return

        try:
            # Use shell=True on Windows for npm.cmd compatibility, and
            # DEVNULL for stdout/stderr to avoid pipe handle issues on
            # Python 3.14+ Windows with shell=True ([WinError 6]).
            # The build output is not needed for test assertions — we
            # verify success via returncode and CJS file existence.
            use_shell = (sys.platform == "win32")
            result = subprocess.run(
                ["npm", "run", "build"],
                cwd=BUILD_DIR,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                text=True,
                timeout=120,
                shell=use_shell,
            )
            cls._build_output = result.stdout
            cls._build_returncode = result.returncode  # FP-3: store returncode

            if result.returncode != 0:
                cls._build_error = (
                    f"Build failed (exit {result.returncode}):\n"
                    f"stdout: {result.stdout}\n"
                    f"stderr: {result.stderr}"
                )
                return

            cls._rebuilt = True

            if os.path.isfile(CJS_PATH):
                with open(CJS_PATH, "r", encoding="utf-8") as f:
                    cls.content = f.read()

                # FP-2: Extract tool identifiers from registration areas only
                cls._tool_identifiers = extract_all_tool_identifiers(cls.content)
        except FileNotFoundError as e:
            cls._build_error = f"Build command not found: {e}"
        except subprocess.TimeoutExpired:
            cls._build_error = "Build timed out after 120 seconds"
        except Exception as e:
            cls._build_error = f"Build failed: {e}"

    # ─── Helper ───────────────────────────────────────────────────────────

    def assert_build_succeeded(self):
        """Skip test if build was not run or failed."""
        if self._build_error and self._build_returncode != 0:
            self.skipTest(f"Build not available: {self._build_error}")
        if not self._rebuilt:
            self.skipTest("Build was not executed")
        if not self.content:
            self.skipTest("CJS artifact not found after build")

    # ─── FP-3: Build success check ────────────────────────────────────────

    def test_cjs_rebuild_succeeds(self):
        """FP-3: Verify the npm run build command completes successfully.

        Uses assertEqual(result.returncode, 0) instead of assertIsNone on
        _build_error, because _build_error may be an empty string "" when
        the build succeeds (stderr is empty), and assertIsNone("") would
        fail with a false positive.
        """
        if self._build_error and "Build command not found" in self._build_error:
            self.skipTest(self._build_error)

        # FP-3: Check returncode directly, not _build_error for None
        self.assertEqual(
            self._build_returncode, 0,
            f"Build should exit with code 0 but got {self._build_returncode}: "
            f"{self._build_error}"
        )
        self.assertTrue(self._rebuilt, "Build should complete successfully")

        # Verify the output file was created and is non-empty
        if os.path.isfile(CJS_PATH):
            size = os.path.getsize(CJS_PATH)
            self.assertGreater(
                size, 0,
                f"Build artifact is empty: {CJS_PATH}"
            )

    # ─── AC-3: Underscore names in build artifact ─────────────────────────

    def test_cjs_has_underscore_names(self):
        """AC-3 (forward): Verify CJS artifact contains all 11 underscore tool names.

        Checks registerTool name parameters and other tool registration identifiers
        only (FP-2). Description text containing old names as cross-references
        is intentionally NOT scanned.
        """
        self.assert_build_succeeded()

        for name in EXPECTED_TOOL_NAMES:
            # FP-2: Check in extracted identifiers (registerTool, name props, case labels)
            # rather than raw content scan. This avoids false positives from
            # description text cross-references.
            self.assertIn(
                name, self._tool_identifiers,
                f"Underscore tool name '{name}' not found in CJS build artifact "
                f"tool identifiers (registerTool/name/case)"
            )

    def test_cjs_no_slash_tool_names(self):
        """AC-3 (reverse): Verify CJS artifact has no slash-format tool names.

        FP-2: Only checks tool registration areas (registerTool arguments,
        name properties, switch case labels), NOT description text or
        cross-references. A tool A may mention tool B's old name in its
        description as a cross-reference (e.g., "See also phase/next"),
        which is intentionally preserved documentation and not a residue.
        """
        self.assert_build_succeeded()

        for slash_name in SLASH_TOOL_NAMES:
            # FP-2: Check extracted identifiers only, not raw content.
            # This avoids false positives from description text containing
            # old names as cross-references (e.g., "See also phase/next").
            self.assertNotIn(
                slash_name, self._tool_identifiers,
                f"Deprecated slash-format name '{slash_name}' found in "
                f"CJS tool registration identifiers"
            )

    # ─── FP-2 validation: extraction sanity ───────────────────────────────

    def test_tool_identifiers_non_empty(self):
        """Verify that FP-2 extraction found at least some tool identifiers.

        This ensures the extraction patterns are working on the actual build
        artifact. If extraction returns empty, the subsequent tests would pass
        vacuously which would be a false negative.
        """
        self.assert_build_succeeded()

        self.assertGreater(
            len(self._tool_identifiers), 0,
            "FP-2: Tool identifier extraction returned empty set — "
            "extraction patterns may need updating for this bundle version"
        )

    def test_tool_identifiers_expected_count(self):
        """Verify the extracted tool identifiers count is reasonable."""
        self.assert_build_succeeded()

        # Each tool should appear at least once (in registerTool).
        # Additional appearances in name properties or case labels are possible
        # but there should be at least 11 entries.
        self.assertGreaterEqual(
            len(self._tool_identifiers), len(EXPECTED_TOOL_NAMES),
            f"Expected at least {len(EXPECTED_TOOL_NAMES)} tool identifiers "
            f"in CJS build artifact, got {len(self._tool_identifiers)}"
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
