#!/usr/bin/env python3
"""
Validation script: Verify no E2E (end-to-end) keyword residues exist in agents/ and templates/.

This is a one-time manual validation script that checks static artifacts.
Does NOT require running the full test suite.

Covers AC-11: E2E keyword residue detection in templates/ and agents/.

Validation checks:
- No "E2E" or "End to End" or "end-to-end" references in agent prompt files
- No E2E test framework configurations in templates
- E2E test command patterns removed from skill routing
"""

import os
import re
import sys


# Directories to scan for E2E residues
SCAN_DIRS = [
    "plugins/dev-team/agents",
    "plugins/dev-team/templates",
]

# E2E related patterns to detect
E2E_PATTERNS = [
    r'\bE2E\b',
    r'\bEnd[- ]to[- ]End\b',
    r'\bend[- ]to[- ]end\b',
    r'\bend_to_end\b',
    r'\be2e\b',
    r'e2e\b.*test',
    r'test.*\be2e\b',
]


class TestNoE2EResidue(unittest.TestCase):
    """AC-11: Verify no E2E keyword residues in agent/template files."""

    def setUp(self):
        self.project_root = os.path.normpath(
            os.path.join(os.path.dirname(os.path.abspath(__file__)),
                         "..", "..", "..", "..")
        )
        self.residues_found = []

    def _scan_file_for_e2e(self, file_path: str) -> list:
        """Scan a single file for E2E patterns. Returns list of (line_number, pattern, content)."""
        findings = []
        try:
            with open(file_path, "r", encoding="utf-8") as f:
                content = f.read()
        except (OSError, UnicodeDecodeError):
            return findings

        for i, line in enumerate(content.splitlines(), 1):
            for pattern in E2E_PATTERNS:
                if re.search(pattern, line, re.IGNORECASE):
                    # Filter out false positives (e.g., "need to" is not E2E)
                    if "need to" in line.lower():
                        continue
                    findings.append((i, pattern, line.strip()))
        return findings

    def test_agents_no_e2e_residue(self):
        """AC-11: Agent prompt files should have no E2E keyword residues."""
        agents_dir = os.path.join(self.project_root, "plugins", "dev-team", "agents")
        if not os.path.isdir(agents_dir):
            self.skipTest(f"agents directory not found: {agents_dir}")

        for filename in os.listdir(agents_dir):
            if filename.endswith(".md"):
                filepath = os.path.join(agents_dir, filename)
                findings = self._scan_file_for_e2e(filepath)
                for line_num, pattern, line in findings:
                    self.residues_found.append(
                        f"{filename}:{line_num}: matched '{pattern}': {line}"
                    )

        if self.residues_found:
            self.fail(
                f"E2E residues found in agent files:\n  " +
                "\n  ".join(self.residues_found)
            )

    def test_templates_no_e2e_residue(self):
        """AC-11: Template files should have no E2E keyword residues."""
        templates_dir = os.path.join(self.project_root, "plugins", "dev-team", "templates")
        if not os.path.isdir(templates_dir):
            self.skipTest(f"templates directory not found: {templates_dir}")

        for root, dirs, files in os.walk(templates_dir):
            for filename in files:
                if filename.endswith(".template") or filename.endswith(".md"):
                    filepath = os.path.join(root, filename)
                    findings = self._scan_file_for_e2e(filepath)
                    for line_num, pattern, line in findings:
                        self.residues_found.append(
                            f"{os.path.relpath(filepath, templates_dir)}:{line_num}: "
                            f"matched '{pattern}': {line}"
                        )

        if self.residues_found:
            self.fail(
                f"E2E residues found in template files:\n  " +
                "\n  ".join(self.residues_found)
            )


if __name__ == "__main__":
    # Allow running as standalone validation script
    if len(sys.argv) > 1 and sys.argv[1] == "--scan":
        project_root = os.path.normpath(
            os.path.join(os.path.dirname(os.path.abspath(__file__)),
                         "..", "..", "..", "..")
        )
        print(f"Scanning for E2E residues in {project_root}...")
        # Manual scan implementation
        for scan_dir in SCAN_DIRS:
            full_path = os.path.join(project_root, scan_dir)
            if not os.path.isdir(full_path):
                print(f"  [SKIP] Directory not found: {full_path}")
                continue
            print(f"  Scanning {scan_dir}/...")
            for root, dirs, files in os.walk(full_path):
                for fname in files:
                    fpath = os.path.join(root, fname)
                    for i, line in enumerate(open(fpath, encoding="utf-8"), 1):
                        for pat in E2E_PATTERNS:
                            if re.search(pat, line, re.IGNORECASE):
                                if "need to" in line.lower():
                                    continue
                                print(f"    [FOUND] {fname}:{i}: {line.strip()}")
        sys.exit(0)

    import unittest
    unittest.main(verbosity=2)
