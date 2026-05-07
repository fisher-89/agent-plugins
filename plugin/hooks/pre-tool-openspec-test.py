#!/usr/bin/env python3
"""
Hook: PreToolUse - Generate executable test files before implementing OpenSpec tasks.

Intercepts Write/Edit tool calls when an active OpenSpec change exists.
Detects project test framework (Jest/Vitest/pytest) and generates executable
test skeletons, not just markdown templates.
"""

import json
import os
import re
import sys

# Import test framework detection utility
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PLUGIN_ROOT = os.path.dirname(SCRIPT_DIR)
UTILS_DIR = os.path.join(PLUGIN_ROOT, "utils")
TEMPLATES_DIR = os.path.join(PLUGIN_ROOT, "templates")

# Add utils to path for import
if UTILS_DIR not in sys.path:
    sys.path.insert(0, UTILS_DIR)

try:
    import importlib.util
    _tf_path = os.path.join(UTILS_DIR, "test-framework.py")
    if os.path.isfile(_tf_path):
        _spec = importlib.util.spec_from_file_location("test_framework", _tf_path)
        _tf_module = importlib.util.module_from_spec(_spec)
        _spec.loader.exec_module(_tf_module)
        detect_test_framework = _tf_module.detect_test_framework
        get_test_file_extension = _tf_module.get_test_file_extension
        get_test_dir = _tf_module.get_test_dir
        HAS_TEST_FRAMEWORK = True
    else:
        HAS_TEST_FRAMEWORK = False
except Exception:
    HAS_TEST_FRAMEWORK = False


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

    # Skip if the file being written is already a test file or under test-reports
    if is_test_file(file_path, cwd):
        output_result("allow", "")
        return

    # Skip if the file is inside the openspec directory itself
    if is_openspec_artifact(file_path, cwd):
        output_result("allow", "")
        return

    changes_dir = os.path.join(cwd, "openspec", "changes")

    # No changes directory
    if not os.path.isdir(changes_dir):
        output_result("allow", "")
        return

    # Find active changes with pending tasks
    active_change = find_active_change(changes_dir)
    if not active_change:
        output_result("allow", "")
        return

    change_name, change_dir = active_change

    # Read tasks to find current pending task
    tasks_info = read_pending_tasks(os.path.join(change_dir, "tasks.md"))
    if not tasks_info:
        output_result("allow", "")
        return

    # Detect test framework
    framework, runner = ("unknown", "")
    test_dir = os.path.join(cwd, "tests")
    test_ext = ".test.js"

    if HAS_TEST_FRAMEWORK:
        framework, runner = detect_test_framework(cwd)
        if framework != "unknown":
            test_ext = get_test_file_extension(framework)
            test_dir = get_test_dir(cwd, framework)

    # Generate executable test files
    generated_files = []
    test_reports_dir = os.path.join(change_dir, "test-reports")
    os.makedirs(test_reports_dir, exist_ok=True)

    if framework in ("jest", "vitest", "mocha"):
        generated_files = generate_jest_tests(
            cwd, test_dir, test_reports_dir, change_name, tasks_info
        )
    elif framework in ("pytest", "unittest", "nose"):
        generated_files = generate_pytest_tests(
            cwd, test_dir, test_reports_dir, change_name, tasks_info
        )
    else:
        # Fallback to markdown template
        generated_files = generate_markdown_template(
            test_reports_dir, change_name, tasks_info
        )

    if not generated_files:
        output_result("allow", "")
        return

    # Inject context telling Claude to implement tests first
    file_list = ", ".join(os.path.basename(f) for f in generated_files)
    context = (
        f"OpenSpec change '{change_name}' is active with framework '{framework}'. "
        f"Executable test skeletons generated: {file_list}. "
        f"TDD GATE: Write test implementations BEFORE writing production code. "
        f"Each test file contains placeholders - fill in assertions for your task. "
        f"Run tests with: {runner}"
    )

    output_result("allow", context)


def is_test_file(file_path, cwd):
    """Check if the file is already a test file."""
    normalized = os.path.normpath(file_path).lower()
    # Common test file patterns
    test_patterns = [
        "test-reports",
        "test_",
        "_test.",
        ".test.",
        ".spec.",
        "__tests__",
        "tests/",
        "/test/",
    ]
    for pattern in test_patterns:
        if pattern in normalized:
            return True
    return False


def is_openspec_artifact(file_path, cwd):
    """Check if the file is an openspec artifact (proposal, tasks, etc.)."""
    normalized = os.path.normpath(file_path)
    openspec_dir = os.path.normpath(os.path.join(cwd, "openspec"))
    return normalized.startswith(openspec_dir)


def find_active_change(changes_dir):
    """Find the first active change with pending tasks.

    Returns (change_name, change_dir) or None.
    """
    try:
        for entry in os.listdir(changes_dir):
            entry_path = os.path.join(changes_dir, entry)
            if not os.path.isdir(entry_path) or entry == "archive":
                continue

            # Check if tasks.md exists and has pending items
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


def read_pending_tasks(tasks_path):
    """Read pending (incomplete) tasks from tasks.md.

    Returns list of task description strings.
    """
    pending = []
    try:
        with open(tasks_path, "r", encoding="utf-8") as f:
            for line in f:
                m = re.match(r"^\s*- \[ \]\s*(.+)", line)
                if m:
                    pending.append(m.group(1).strip())
    except OSError:
        pass
    return pending


def generate_jest_tests(cwd, test_dir, test_reports_dir, change_name, pending_tasks):
    """Generate executable Jest/Vitest test files for each pending task.

    Supports incremental updates: only generates tests for new tasks.
    Returns list of generated file paths.
    """
    generated = []
    marker_path = os.path.join(test_reports_dir, f".jest-skeletons-{change_name}")

    # Read existing generated tests from marker file
    existing_tests = set()
    if os.path.isfile(marker_path):
        with open(marker_path, "r", encoding="utf-8") as f:
            existing_tests = set(line.strip() for line in f if line.strip())

    for i, task in enumerate(pending_tasks, 1):
        module_name = extract_module_name(task)
        test_filename = f"{module_name}.test.js"
        test_file_path = os.path.join(test_dir, test_filename)

        # Skip if test file already exists (either on disk or in marker)
        if os.path.isfile(test_file_path) or test_file_path in existing_tests:
            continue

        keywords = extract_keywords(task)
        primary_kw = keywords[0] if keywords else "feature"

        content = f"""// {test_filename}
// Auto-generated by wps-claude-plugin
// Change: {change_name}
// Task {i}: {task}

describe('{module_name}', () => {{

  // === Task {i}: {task} ===

  describe('{task}', () => {{

    // --- Happy Path ---
    test('should correctly handle {primary_kw}', () => {{
      // Arrange
      // const input = ...;

      // Act
      // const result = functionUnderTest(input);

      // Assert
      // expect(result).toBe(expected);
      expect(true).toBe(true); // TODO: Replace with actual test
    }});

    // --- Error Cases ---
    test('should handle invalid {primary_kw} gracefully', () => {{
      // Arrange
      // const invalidInput = ...;

      // Act & Assert
      // expect(() => functionUnderTest(invalidInput)).toThrow();
      expect(true).toBe(true); // TODO: Replace with actual test
    }});

    // --- Edge Cases ---
    test('should handle empty/null {primary_kw}', () => {{
      // Arrange
      // const emptyInput = null;

      // Act & Assert
      // expect(() => functionUnderTest(emptyInput)).toThrow();
      expect(true).toBe(true); // TODO: Replace with actual test
    }});

  }});

}});
"""
        os.makedirs(test_dir, exist_ok=True)
        with open(test_file_path, "w", encoding="utf-8") as f:
            f.write(content)
        generated.append(test_file_path)

    # Update marker file (append new tests)
    if generated:
        all_tests = existing_tests.union(generated)
        with open(marker_path, "w", encoding="utf-8") as f:
            f.write("\n".join(all_tests))

    # Update markdown test-cases report incrementally
    _update_test_cases_md(test_reports_dir, change_name, pending_tasks, "jest")
    return generated


def generate_pytest_tests(cwd, test_dir, test_reports_dir, change_name, pending_tasks):
    """Generate executable pytest test files for each pending task.

    Supports incremental updates: only generates tests for new tasks.
    Returns list of generated file paths.
    """
    generated = []
    marker_path = os.path.join(test_reports_dir, f".pytest-skeletons-{change_name}")

    # Read existing generated tests from marker file
    existing_tests = set()
    if os.path.isfile(marker_path):
        with open(marker_path, "r", encoding="utf-8") as f:
            existing_tests = set(line.strip() for line in f if line.strip())

    for i, task in enumerate(pending_tasks, 1):
        module_name = extract_module_name(task)
        test_filename = f"test_{module_name}.py"
        test_file_path = os.path.join(test_dir, test_filename)

        # Skip if test file already exists (either on disk or in marker)
        if os.path.isfile(test_file_path) or test_file_path in existing_tests:
            continue

        keywords = extract_keywords(task)
        primary_kw = keywords[0] if keywords else "feature"
        class_name = "".join(word.capitalize() for word in module_name.split("_"))

        content = f"""# {test_filename}
# Auto-generated by wps-claude-plugin
# Change: {change_name}
# Task {i}: {task}

import pytest
from unittest.mock import Mock, patch


class Test{class_name}:
    \"\"\"Tests for {module_name} module.\"\"\"

    # === Task {i}: {task} ===

    def test_handles_{primary_kw}_correctly(self):
        \"\"\"Test that {primary_kw} is handled correctly.\"\"\"
        # Arrange
        # input_data = ...

        # Act
        # result = function_under_test(input_data)

        # Assert
        # assert result == expected
        assert True  # TODO: Replace with actual test

    def test_handles_invalid_{primary_kw}(self):
        \"\"\"Test that invalid {primary_kw} is handled gracefully.\"\"\"
        # Arrange
        # invalid_input = ...

        # Act & Assert
        # with pytest.raises(ValueError):
        #     function_under_test(invalid_input)
        assert True  # TODO: Replace with actual test

    def test_handles_empty_{primary_kw}(self):
        \"\"\"Test that empty/null {primary_kw} is handled correctly.\"\"\"
        # Arrange & Act & Assert
        # with pytest.raises(TypeError):
        #     function_under_test(None)
        assert True  # TODO: Replace with actual test


# === Integration Tests ===

class Test{class_name}Integration:
    \"\"\"Integration tests for {module_name}.\"\"\"

    def test_end_to_end_flow(self):
        \"\"\"Test the complete flow for {task}.\"\"\"
        # Arrange
        # Act
        # Assert
        assert True  # TODO: Replace with actual test
"""
        os.makedirs(test_dir, exist_ok=True)
        with open(test_file_path, "w", encoding="utf-8") as f:
            f.write(content)
        generated.append(test_file_path)

    # Update marker file
    if generated:
        all_tests = existing_tests.union(generated)
        with open(marker_path, "w", encoding="utf-8") as f:
            f.write("\n".join(all_tests))

    _update_test_cases_md(test_reports_dir, change_name, pending_tasks, "pytest")
    return generated


def generate_markdown_template(test_reports_dir, change_name, pending_tasks):
    """Fallback: generate markdown test case template when no framework detected.

    Returns list of generated file paths.
    """
    template_path = os.path.join(test_reports_dir, f"test-cases-{change_name}.md")
    if os.path.isfile(template_path):
        return []

    _update_test_cases_md(test_reports_dir, change_name, pending_tasks, "unknown")
    return [template_path]


def _update_test_cases_md(test_reports_dir, change_name, pending_tasks, framework):
    """Update markdown test case report incrementally in test-reports/.

    Only appends new tasks that aren't already in the report.
    """
    template_path = os.path.join(test_reports_dir, f"test-cases-{change_name}.md")

    # Read existing task headings to determine what's already in the report
    existing_task_headings = set()
    if os.path.isfile(template_path):
        with open(template_path, "r", encoding="utf-8") as f:
            for line in f:
                m = re.match(r"^### Task \d+: (.+)$", line)
                if m:
                    existing_task_headings.add(m.group(1).strip())

    # Find new tasks not yet in the report
    new_tasks = []
    for task in pending_tasks:
        if task not in existing_task_headings:
            new_tasks.append(task)

    if not new_tasks and os.path.isfile(template_path):
        return  # Nothing new to add

    # Generate content for new tasks
    new_lines = []
    for i, task in enumerate(pending_tasks, 1):
        if task not in new_tasks:
            continue
        keywords = extract_keywords(task)
        module_name = extract_module_name(task)

        if framework in ("jest", "vitest", "mocha"):
            test_file_label = f"**Test file**: `{module_name}.test.js`"
        elif framework in ("pytest", "unittest", "nose"):
            test_file_label = f"**Test file**: `test_{module_name}.py`"
        else:
            test_file_label = "**Test file**: (no framework detected)"

        new_lines.extend([
            f"### Task {i}: {task}",
            "",
            test_file_label,
            "",
            "#### Unit Tests",
        ])
        for j, kw in enumerate(keywords[:3], 1):
            new_lines.append(f"- [ ] Test {j}: Should correctly handle {kw}")
        new_lines.extend([
            f"- [ ] Test {len(keywords[:3]) + 1}: Should return error for invalid {keywords[0] if keywords else 'input'}",
            "",
            "#### Integration Tests",
            f"- [ ] Test 1: End-to-end flow for {task}",
            "",
            "#### Edge Cases",
            f"- [ ] Test 1: Empty or null {keywords[0] if keywords else 'input'}",
            f"- [ ] Test 2: Concurrent access to {keywords[0] if keywords else 'resource'}",
            "",
        ])

    if not os.path.isfile(template_path):
        # Create new report
        lines = [
            f"# Test Cases: {change_name}",
            "",
            f"> Framework: {framework}",
            "> Generated: auto",
            "",
            "## Test Cases by Task",
            "",
        ]
        lines.extend(new_lines)
        lines.extend([
            "---",
            "",
            "*This report was auto-generated by the OpenSpec PreToolUse hook.*",
        ])
        with open(template_path, "w", encoding="utf-8") as f:
            f.write("\n".join(lines))
    else:
        # Append new tasks to existing report
        with open(template_path, "r", encoding="utf-8") as f:
            content = f.read()

        # Insert new tasks before the "---" separator at the end
        separator = "\n---\n"
        if separator in content:
            parts = content.split(separator, 1)
            updated = parts[0] + "\n" + "\n".join(new_lines) + separator + parts[1]
        else:
            updated = content + "\n" + "\n".join(new_lines)

        with open(template_path, "w", encoding="utf-8") as f:
            f.write(updated)


def extract_module_name(task_description):
    """Extract a module name from task description for test file naming.

    Examples:
        "Implement user authentication" -> "user_authentication"
        "Add JWT token validation" -> "jwt_token_validation"
        "Create user model" -> "user_model"
    """
    stop_words = {
        "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for",
        "of", "with", "by", "from", "is", "are", "was", "were", "be", "been",
        "being", "have", "has", "had", "do", "does", "did", "will", "would",
        "could", "should", "may", "might", "shall", "can", "set", "add",
        "implement", "create", "setup", "write", "build", "configure",
        "make", "update", "remove", "delete", "get", "put", "post",
    }

    words = re.split(r"[\s/\-(),]+", task_description.lower())
    meaningful = [w for w in words if len(w) >= 2 and w not in stop_words]

    if meaningful:
        return "_".join(meaningful[:3])  # Use first 3 meaningful words
    return "module"


def extract_keywords(task_description):
    """Extract key nouns/concepts from a task description for test generation."""
    # Simple keyword extraction: split on common delimiters, filter short words
    # and common verbs/prepositions
    stop_words = {
        "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for",
        "of", "with", "by", "from", "is", "are", "was", "were", "be", "been",
        "being", "have", "has", "had", "do", "does", "did", "will", "would",
        "could", "should", "may", "might", "shall", "can", "set", "add",
        "implement", "create", "setup", "write", "build", "configure",
    }

    # Split on common delimiters
    words = re.split(r"[\s/\-(),]+", task_description.lower())
    keywords = []
    seen = set()

    for word in words:
        word = word.strip()
        if len(word) >= 3 and word not in stop_words and word not in seen:
            keywords.append(word)
            seen.add(word)

    return keywords


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
