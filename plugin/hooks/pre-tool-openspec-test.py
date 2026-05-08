#!/usr/bin/env python3
"""
Hook: PreToolUse - Inject TDD reminder for OpenSpec changes.

Simplified version: Only injects context reminder, does not generate test files.
Test generation is handled by test-generator.py in apply-change workflow.
"""

import json
import os
import re
import sys


def main():
    # Read JSON input from stdin
    input_data = json.load(sys.stdin)
    tool_name = input_data.get("tool_name", "")
    tool_input = input_data.get("tool_input", {})
    cwd = input_data.get("cwd", "")

    # Only intercept Write and Edit
    if tool_name not in ("Write", "Edit"):
        output_result("allow", "")
        return

    file_path = tool_input.get("file_path", "")

    # Skip if the file being written is already a test file
    if is_test_file(file_path, cwd):
        output_result("allow", "")
        return

    # Skip if the file is inside the openspec directory
    if is_openspec_artifact(file_path, cwd):
        output_result("allow", "")
        return

    changes_dir = os.path.join(cwd, "openspec", "changes")

    # No changes directory
    if not os.path.isdir(changes_dir):
        output_result("allow", "")
        return

    # Find active change with pending tasks
    active_change = find_active_change(changes_dir)
    if not active_change:
        output_result("allow", "")
        return

    change_name, change_dir = active_change

    # Inject TDD reminder context
    context = (
        f"OpenSpec change '{change_name}' is active. "
        f"TDD: Write tests BEFORE production code. "
        f"Tests should be in the same directory as source files."
    )

    output_result("allow", context)


def is_test_file(file_path, cwd):
    """Check if the file is already a test file."""
    normalized = os.path.normpath(file_path).lower()
    test_patterns = [
        "test-reports",
        "test_",
        "_test.",
        ".test.",
        ".spec.",
        "__tests__",
        "tests/",
        "/test/",
        "_tests.rs",  # Rust test pattern
    ]
    for pattern in test_patterns:
        if pattern in normalized:
            return True
    return False


def is_openspec_artifact(file_path, cwd):
    """Check if the file is an openspec artifact."""
    normalized = os.path.normpath(file_path)
    openspec_dir = os.path.normpath(os.path.join(cwd, "openspec"))
    return normalized.startswith(openspec_dir)


def find_active_change(changes_dir):
    """Find the first active change with pending tasks."""
    try:
        for entry in os.listdir(changes_dir):
            entry_path = os.path.join(changes_dir, entry)
            if not os.path.isdir(entry_path) or entry == "archive":
                continue

            tasks_path = os.path.join(entry_path, "tasks.md")
            if os.path.isfile(tasks_path):
                total, done = count_tasks(tasks_path)
                if done < total:
                    return (entry, entry_path)
    except OSError:
        pass

    return None


def count_tasks(tasks_path):
    """Count total and completed tasks in a tasks.md file."""
    total = 0
    done = 0
    try:
        with open(tasks_path, "r", encoding="utf-8") as f:
            for line in f:
                if re.match(r"^\s*- \[", line):
                    total += 1
                    if re.match(r"^\s*- \[x\]", line):
                        done += 1
    except OSError:
        pass
    return total, done


def output_result(decision, additional_context):
    """Output the hook result as JSON."""
    result = {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": decision,
        }
    }
    if additional_context:
        result["hookSpecificOutput"]["additionalContext"] = additional_context
    json.dump(result, sys.stdout)


if __name__ == "__main__":
    main()
