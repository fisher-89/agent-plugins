#!/usr/bin/env python3
"""
Verify slim-phase-skills skill file properties.

Checks:
1. Line count <= 60 for each of 7 skill files
2. Each skill contains `eval-check --change` call
3. All Agent prompts are single-line (no embedded newlines)
4. No Phase Pattern sections (DESIGN/EXECUTION/EVALUATOR-ONLY)
5. phase-implement contains lint-runner.py call
6. phase-requirements contains Branch A/B markers
7. No references to requirements-planner as subagent
"""

import os
import re
import sys

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
SKILLS_DIR = os.path.join(PROJECT_ROOT, "plugins", "dev-team", "skills")

SKILL_NAMES = [
    "phase-requirements",
    "phase-test-design",
    "phase-dev-proposal",
    "phase-test-gen",
    "phase-implement",
    "phase-code-review",
    "phase-acceptance",
]

PHASE_PATTERNS = [
    "## DESIGN Phase Pattern",
    "## EXECUTION Phase Pattern",
    "## EVALUATOR-ONLY Pattern",
]

errors = []


def get_skill_path(name):
    return os.path.join(SKILLS_DIR, name, "SKILL.md")


def count_lines(filepath):
    with open(filepath, "r", encoding="utf-8") as f:
        return sum(1 for _ in f)


def read_file(filepath):
    with open(filepath, "r", encoding="utf-8") as f:
        return f.read()


def has_eval_check(content):
    return "eval-check --change" in content


def has_phase_pattern(content):
    for pattern in PHASE_PATTERNS:
        if pattern in content:
            return True, pattern
    return False, None


def has_lint_runner(content):
    return "lint-runner.py" in content


def has_branch_markers(content):
    return "Branch A" in content and "Branch B" in content


def has_requirements_planner_ref(content):
    return "requirements-planner" in content


def check_single_line_prompts(filepath):
    """Check that all Agent prompts are single-line strings."""
    content = read_file(filepath)
    lines = content.split("\n")
    in_agent_block = False
    in_prompt = False
    prompt_lines = []
    issues = []

    for i, line in enumerate(lines, 1):
        stripped = line.strip()

        if stripped.startswith("Agent({"):
            in_agent_block = True
            in_prompt = False
            prompt_lines = []
            continue

        if in_agent_block:
            if stripped.startswith("prompt:"):
                in_prompt = True
                # Check if it's a single-line prompt
                prompt_value = stripped[len("prompt:"):].strip()
                if prompt_value.startswith('"') and prompt_value.endswith('"'):
                    # Single-line prompt, no issues
                    pass
                elif prompt_value.startswith("'") and prompt_value.endswith("'"):
                    # Single-line prompt, no issues
                    pass
                else:
                    # Could be multi-line, need to check if next lines are indented
                    prompt_lines.append(i)
            elif in_prompt:
                if stripped == "})" or stripped.startswith("})"):
                    in_agent_block = False
                    in_prompt = False
                    # Check if prompt was multi-line
                    if len(prompt_lines) > 0:
                        issues.append(
                            f"  Multi-line prompt detected near line {prompt_lines[0]}"
                        )
                elif stripped and not stripped.startswith("//"):
                    prompt_lines.append(i)
            elif stripped == "})":
                in_agent_block = False
            elif stripped == "}":
                in_agent_block = False

    return issues


def main():
    all_checks_passed = True

    # Check 1 & 2: Line count and eval-check existence
    print("=" * 60)
    print("Check 1: Line count <= 60")
    print("Check 2: eval-check --change presence")
    print("-" * 60)

    for name in SKILL_NAMES:
        path = get_skill_path(name)
        if not os.path.isfile(path):
            errors.append(f"MISSING: {path}")
            all_checks_passed = False
            continue

        content = read_file(path)
        lines = count_lines(path)

        # Check 1: line count
        if lines > 60:
            errors.append(f"LINE COUNT FAIL: {name} has {lines} lines (> 60)")
            all_checks_passed = False
            print(f"  {name}: {lines} lines -- FAIL (> 60)")
        else:
            print(f"  {name}: {lines} lines -- PASS")

        # Check 2: eval-check
        if has_eval_check(content):
            print(f"  {name}: eval-check --change -- PASS")
        else:
            errors.append(f"EVAL_CHECK FAIL: {name} missing 'eval-check --change'")
            all_checks_passed = False
            print(f"  {name}: eval-check --change -- FAIL")

    # Check 3: Single-line prompts
    print()
    print("=" * 60)
    print("Check 3: Agent prompts are single-line")
    print("-" * 60)

    for name in SKILL_NAMES:
        path = get_skill_path(name)
        if not os.path.isfile(path):
            continue

        issues = check_single_line_prompts(path)
        if issues:
            errors.append(f"PROMPT FORMAT FAIL: {name}")
            for issue in issues:
                errors.append(f"  {issue}")
            all_checks_passed = False
            for issue in issues:
                print(f"  {name}: {issue}")
        else:
            print(f"  {name}: all prompts single-line -- PASS")

    # Check 4: No Phase Pattern sections
    print()
    print("=" * 60)
    print("Check 4: No Phase Pattern sections")
    print("-" * 60)

    for name in SKILL_NAMES:
        path = get_skill_path(name)
        if not os.path.isfile(path):
            continue

        content = read_file(path)
        found, pattern = has_phase_pattern(content)
        if found:
            errors.append(f"PHASE PATTERN FAIL: {name} contains '{pattern}'")
            all_checks_passed = False
            print(f"  {name}: contains '{pattern}' -- FAIL")
        else:
            print(f"  {name}: no Phase Pattern -- PASS")

    # Check 5: phase-implement has lint-runner
    print()
    print("=" * 60)
    print("Check 5: phase-implement has lint-runner.py")
    print("-" * 60)

    imp_path = get_skill_path("phase-implement")
    if os.path.isfile(imp_path):
        content = read_file(imp_path)
        if has_lint_runner(content):
            print("  phase-implement: lint-runner.py -- PASS")
        else:
            errors.append("LINT_RUNNER FAIL: phase-implement missing lint-runner.py")
            all_checks_passed = False
            print("  phase-implement: lint-runner.py -- FAIL")

    # Check 6: phase-requirements has Branch A/B
    print()
    print("=" * 60)
    print("Check 6: phase-requirements has Branch A/B markers")
    print("-" * 60)

    req_path = get_skill_path("phase-requirements")
    if os.path.isfile(req_path):
        content = read_file(req_path)
        if has_branch_markers(content):
            print("  phase-requirements: Branch A/B -- PASS")
        else:
            errors.append(
                "BRANCH_MARKER FAIL: phase-requirements missing Branch A/B markers"
            )
            all_checks_passed = False
            print("  phase-requirements: Branch A/B -- FAIL")

    # Check 7: No requirements-planner references
    print()
    print("=" * 60)
    print("Check 7: No requirements-planner references in skills")
    print("-" * 60)

    for name in SKILL_NAMES:
        path = get_skill_path(name)
        if not os.path.isfile(path):
            continue

        content = read_file(path)
        if has_requirements_planner_ref(content):
            errors.append(
                f"REQUIREMENTS_PLANNER REF FAIL: {name} references requirements-planner"
            )
            all_checks_passed = False
            print(f"  {name}: references requirements-planner -- FAIL")
        else:
            print(f"  {name}: no requirements-planner ref -- PASS")

    # Summary
    print()
    print("=" * 60)
    if all_checks_passed:
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
