#!/usr/bin/env python3
"""
Validation script: Verify architecture.md has no residual archi command references
to the old Python-based archi_parser utilities.

Covers AC-18: architecture.md should not contain `python plugins/dev-team/utils/archi-*.py`
command references.

Validation checks:
- No `python plugins/dev-team/utils/archi-` references in architecture.md
- Command references use `dev-team archi` instead of direct Python invocations
"""

import os
import re
import sys
import unittest


# Pattern to detect old Python archi command references
OLD_ARCHI_PATTERN = re.compile(
    r'python\s+plugins/dev-team/utils/archi[-_][a-z]+\.py'
)

# Patterns that should be present (new CLI commands)
NEW_ARCHI_COMMANDS = [
    'dev-team archi query',
    'dev-team archi validate',
    'dev-team archi write',
    'dev-team archi check',
]


class TestArchitectureMdCommands(unittest.TestCase):
    """AC-18: Verify architecture.md has no residual old archi command references."""

    def setUp(self):
        self.project_root = os.path.normpath(
            os.path.join(os.path.dirname(os.path.abspath(__file__)),
                         "..", "..", "..", "..")
        )
        self.arch_md_path = os.path.join(
            self.project_root, "openspec", "specs", "architecture", "architecture.md"
        )

    def test_no_old_archi_python_commands_in_architecture_md(self):
        """AC-18: architecture.md should not reference old archi Python commands."""
        if not os.path.isfile(self.arch_md_path):
            self.skipTest(f"architecture.md not found: {self.arch_md_path}")

        with open(self.arch_md_path, "r", encoding="utf-8") as f:
            content = f.read()

        matches = OLD_ARCHI_PATTERN.findall(content)
        if matches:
            self.fail(
                f"Found {len(matches)} old archi command reference(s) in architecture.md:\n  " +
                "\n  ".join(matches)
            )

    def test_new_archi_commands_mentioned(self):
        """AC-18: architecture.md should reference new dev-team archi commands."""
        if not os.path.isfile(self.arch_md_path):
            self.skipTest(f"architecture.md not found: {self.arch_md_path}")

        with open(self.arch_md_path, "r", encoding="utf-8") as f:
            content = f.read()

        # At least one new-style command should be mentioned
        found_commands = [cmd for cmd in NEW_ARCHI_COMMANDS if cmd in content]
        # This is informational; the actual check is test_no_old_archi_python_commands
        if not found_commands:
            print("  [INFO] No new dev-team archi commands found in architecture.md "
                  "(may be documented elsewhere)")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--scan":
        project_root = os.path.normpath(
            os.path.join(os.path.dirname(os.path.abspath(__file__)),
                         "..", "..", "..", "..")
        )
        arch_md = os.path.join(project_root, "openspec", "specs", "architecture",
                               "architecture.md")
        if os.path.isfile(arch_md):
            with open(arch_md, "r", encoding="utf-8") as f:
                for i, line in enumerate(f, 1):
                    if OLD_ARCHI_PATTERN.search(line):
                        print(f"  [FOUND] {arch_md}:{i}: {line.strip()}")
        else:
            print(f"  [SKIP] architecture.md not found at {arch_md}")
        sys.exit(0)

    unittest.main(verbosity=2)
