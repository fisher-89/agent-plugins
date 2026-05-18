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

# Import shared utilities
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PLUGIN_ROOT = os.path.dirname(SCRIPT_DIR)
UTILS_DIR = os.path.join(PLUGIN_ROOT, "utils")

if UTILS_DIR not in sys.path:
    sys.path.insert(0, UTILS_DIR)

from hook_output import output_pre_tool_use

try:
    import importlib.util
    _ac_path = os.path.join(UTILS_DIR, "active-change.py")
    if os.path.isfile(_ac_path):
        _ac_spec = importlib.util.spec_from_file_location("active_change", _ac_path)
        _ac_module = importlib.util.module_from_spec(_ac_spec)
        _ac_spec.loader.exec_module(_ac_module)
        find_active_change = _ac_module.find_active_change
        count_tasks = _ac_module.count_tasks
    else:
        # Fallback: inline implementations
        def find_active_change(changes_dir, cwd=""):
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
except Exception:
    def find_active_change(changes_dir, cwd=""):
        return None
    def count_tasks(tasks_path):
        return 0, 0


def main():
    # Read JSON input from stdin
    input_data = json.load(sys.stdin)
    tool_name = input_data.get("tool_name", "")
    tool_input = input_data.get("tool_input", {})
    cwd = input_data.get("cwd", "")

    # Only intercept Write and Edit
    if tool_name not in ("Write", "Edit"):
        output_pre_tool_use("allow", "")
        return

    file_path = tool_input.get("file_path", "")

    # Skip if the file being written is already a test file
    if is_test_file(file_path, cwd):
        output_pre_tool_use("allow", "")
        return

    # Skip if the file is inside the openspec directory
    if is_openspec_artifact(file_path, cwd):
        output_pre_tool_use("allow", "")
        return

    # Skip design artifacts (.md, .md.template, .json schemas) — evaluated by Evaluator agents
    if is_design_artifact(file_path):
        output_pre_tool_use("allow", "")
        return

    changes_dir = os.path.join(cwd, "openspec", "changes")

    # No changes directory
    if not os.path.isdir(changes_dir):
        output_pre_tool_use("allow", "")
        return

    # Find active change (uses shared module with priority ordering)
    active_change = find_active_change(changes_dir, cwd)
    if not active_change:
        output_pre_tool_use("allow", "")
        return

    change_name, change_dir = active_change

    # Check if a corresponding test file exists
    test_file = map_source_to_test(file_path, cwd)
    test_exists = test_file and os.path.isfile(test_file)

    # Build context based on test file existence
    relative_path = os.path.relpath(file_path, cwd) if cwd else file_path

    if test_exists:
        # Test file exists — brief reminder
        context = (
            f"OpenSpec change '{change_name}' is active. "
            f"Test file found: {os.path.relpath(test_file, cwd) if cwd else test_file}. "
            f"Ensure tests cover the changes you are making."
        )
    else:
        # No test file — strong TDD GATE instruction
        test_gen_path = os.path.join(UTILS_DIR, "test-generator.py")
        gen_command = ""
        if os.path.isfile(test_gen_path):
            gen_command = (
                f"python plugin/utils/test-generator.py --source '{relative_path}' --project-root '{cwd}' "
                f"to generate a test skeleton. "
            )

        context = (
            f"OpenSpec change '{change_name}' is active. "
            f"TDD GATE: No test file found for '{relative_path}'. "
            f"BEFORE implementing, you MUST run: "
            f"{gen_command}"
            f"Write tests first, then implement."
        )

    output_pre_tool_use("allow", context)


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


def is_design_artifact(file_path):
    """Check if the file is a design artifact that doesn't need unit tests.

    Design artifacts (.md, .md.template, .json schemas) are evaluated by
    Evaluator agents in the PGE workflow, not by traditional unit tests.
    """
    normalized = os.path.normpath(file_path).lower()
    # Template files (Planner output templates)
    if normalized.endswith(".md.template") or normalized.endswith(".json.template"):
        return True
    # Agent definition files (Planner, Generator, Evaluator prompts)
    if "agents" in normalized.split(os.sep) and normalized.endswith(".md"):
        return True
    # Skill definition files
    if "skills" in normalized.split(os.sep) and normalized.endswith(".md"):
        return True
    # JSON schema files (eval.schema.json, checklist.schema.json)
    if "templates" in normalized.split(os.sep) and normalized.endswith(".schema.json"):
        return True
    return False


def map_source_to_test(file_path, cwd):
    """Map source file path to expected test file path (colocated convention).

    Args:
        file_path: Path to the source file
        cwd: Current working directory

    Returns:
        Expected test file path or None if cannot determine
    """
    if not file_path:
        return None

    # Get the file extension and base name
    _, ext = os.path.splitext(file_path)
    base_name = os.path.basename(file_path)
    dir_path = os.path.dirname(file_path)

    # Map by extension
    if ext in (".js", ".jsx", ".mjs", ".cjs"):
        # JavaScript: file.js -> file.test.js
        name_without_ext = os.path.splitext(base_name)[0]
        return os.path.join(dir_path, f"{name_without_ext}.test.js")

    elif ext in (".ts", ".tsx"):
        # TypeScript: file.ts -> file.test.ts
        name_without_ext = os.path.splitext(base_name)[0]
        return os.path.join(dir_path, f"{name_without_ext}.test.ts")

    elif ext == ".py":
        # Python: auth.py -> test_auth.py
        name_without_ext = os.path.splitext(base_name)[0]
        return os.path.join(dir_path, f"test_{name_without_ext}.py")

    elif ext == ".rs":
        # Rust: auth.rs -> auth_tests.rs (uses #[path] import)
        name_without_ext = os.path.splitext(base_name)[0]
        return os.path.join(dir_path, f"{name_without_ext}_tests.rs")

    elif ext in (".go",):
        # Go: file.go -> file_test.go
        name_without_ext = os.path.splitext(base_name)[0]
        return os.path.join(dir_path, f"{name_without_ext}_test.go")

    # Unknown extension — cannot determine test file
    return None


if __name__ == "__main__":
    main()
