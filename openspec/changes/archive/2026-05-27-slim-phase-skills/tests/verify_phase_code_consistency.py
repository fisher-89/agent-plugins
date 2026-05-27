#!/usr/bin/env python3
"""
Verify phase code consistency for slim-phase-skills.

Checks:
1. implementation-evaluator.md uses `05-implement` (not `05-implementation`)
2. openspec-archive-change/SKILL.md uses `dev-team eval-check` (not `eval-check.py`)
"""

import os
import re
import sys

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))

FILES_TO_CHECK = [
    {
        "path": os.path.join(
            PROJECT_ROOT, "plugins", "dev-team", "agents", "implementation-evaluator.md"
        ),
        "label": "implementation-evaluator.md",
        "must_contain": "--phase 05-implement",
        "must_not_contain": "--phase 05-implementation",
    },
    {
        "path": os.path.join(
            PROJECT_ROOT, "plugins", "dev-team", "skills", "openspec-archive-change", "SKILL.md"
        ),
        "label": "openspec-archive-change/SKILL.md",
        "must_contain": "dev-team eval-check",
        "must_not_contain": "eval-check.py",
    },
]

errors = []


def main():
    all_passed = True

    print("=" * 60)
    print("Check: Phase code consistency")
    print("-" * 60)

    for entry in FILES_TO_CHECK:
        filepath = entry["path"]
        label = entry["label"]
        must_contain = entry["must_contain"]
        must_not_contain = entry["must_not_contain"]

        if not os.path.isfile(filepath):
            errors.append(f"MISSING: {label} at {filepath}")
            all_passed = False
            print(f"  {label}: FILE NOT FOUND -- FAIL")
            continue

        with open(filepath, "r", encoding="utf-8") as f:
            content = f.read()

        # Check must_contain
        if must_contain in content:
            print(f"  {label}: contains '{must_contain}' -- PASS")
        else:
            errors.append(f"MISSING_REF: {label} should contain '{must_contain}'")
            all_passed = False
            print(f"  {label}: missing '{must_contain}' -- FAIL")

        # Check must_not_contain
        if must_not_contain not in content:
            print(f"  {label}: no '{must_not_contain}' -- PASS")
        else:
            errors.append(
                f"DEPRECATED_REF: {label} still contains '{must_not_contain}'"
            )
            all_passed = False
            print(f"  {label}: contains '{must_not_contain}' -- FAIL")

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
