#!/usr/bin/env python3
"""
Verify deleted files for slim-phase-skills.

Checks:
1. plugins/dev-team/skills/code-review/ directory does not exist
2. plugins/dev-team/agents/code-review.md file does not exist
3. plugins/dev-team/utils/eval-check.py file does not exist
4. plugins/dev-team/agents/requirements-planner.md file does not exist
5. Scan other files for residual references to these deleted files
"""

import os
import re
import sys

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))

DELETED_PATHS = [
    os.path.join(PROJECT_ROOT, "plugins", "dev-team", "skills", "code-review"),
    os.path.join(PROJECT_ROOT, "plugins", "dev-team", "agents", "code-review.md"),
    os.path.join(PROJECT_ROOT, "plugins", "dev-team", "utils", "eval-check.py"),
    os.path.join(PROJECT_ROOT, "plugins", "dev-team", "agents", "requirements-planner.md"),
]

# Patterns to scan for residual references
RESIDUAL_PATTERNS = {
    "eval-check.py": r"eval-check\.py",
    "code-review skill": r"skills/code-review",
    "requirements-planner": r"requirements-planner",
}

SCAN_DIRECTORIES = [
    os.path.join(PROJECT_ROOT, "plugins", "dev-team", "skills"),
    os.path.join(PROJECT_ROOT, "plugins", "dev-team", "agents"),
]

errors = []


def scan_for_residuals():
    """Scan files for residual references to deleted files."""
    found_refs = []
    for scan_dir in SCAN_DIRECTORIES:
        if not os.path.isdir(scan_dir):
            continue
        for root, dirs, files in os.walk(scan_dir):
            for fname in files:
                if not fname.endswith((".md", ".py", ".ts", ".js", ".sh")):
                    continue
                filepath = os.path.join(root, fname)
                try:
                    with open(filepath, "r", encoding="utf-8") as f:
                        content = f.read()
                except (IOError, UnicodeDecodeError):
                    continue

                for label, pattern in RESIDUAL_PATTERNS.items():
                    if re.search(pattern, content):
                        # calc relative path
                        rel_path = os.path.relpath(filepath, PROJECT_ROOT)
                        found_refs.append(
                            f"  '{rel_path}' references '{label}' ({pattern})"
                        )
    return found_refs


def main():
    all_passed = True

    print("=" * 60)
    print("Check: Deleted files are absent")
    print("-" * 60)

    for path in DELETED_PATHS:
        exists = os.path.exists(path)
        rel_path = os.path.relpath(path, PROJECT_ROOT)
        if exists:
            errors.append(f"STILL EXISTS: {rel_path}")
            all_passed = False
            print(f"  {rel_path}: STILL EXISTS -- FAIL")
        else:
            print(f"  {rel_path}: confirmed deleted -- PASS")

    print()
    print("=" * 60)
    print("Check: Scan for residual references")
    print("-" * 60)

    found_refs = scan_for_residuals()
    if found_refs:
        errors.append("RESIDUAL REFERENCES FOUND:")
        for ref in found_refs:
            errors.append(ref)
        all_passed = False
        for ref in found_refs:
            print(f"  {ref}")
    else:
        print("  No residual references found -- PASS")

    print()
    print("=" * 60)
    if all_passed:
        print("RESULT: ALL CHECKS PASSED")
        sys.exit(0)
    else:
        print("RESULT: SOME CHECKS FAILED")
        print()
        for err in errors:
            print(f"  - {err}")
        sys.exit(1)


if __name__ == "__main__":
    main()
