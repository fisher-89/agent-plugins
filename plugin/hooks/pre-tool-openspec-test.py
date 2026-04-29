#!/usr/bin/env python3
"""
Hook: PreToolUse - Generate test case templates before implementing OpenSpec tasks.

Intercepts Write/Edit tool calls when an active OpenSpec change exists.
If the target file is not a test file and no test cases have been generated yet,
injects additionalContext instructing Claude to generate test cases first.
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

    # Check if test cases already generated for this change
    test_reports_dir = os.path.join(change_dir, "test-reports")
    if os.path.isdir(test_reports_dir):
        # Check if there are any .md files in test-reports
        test_files = [f for f in os.listdir(test_reports_dir) if f.endswith(".md")]
        if test_files:
            # Test cases already exist, allow without extra context
            output_result("allow", "")
            return

    # Read tasks to find current pending task
    tasks_info = read_pending_tasks(os.path.join(change_dir, "tasks.md"))

    # Generate test case template
    template = generate_test_template(change_name, change_dir, tasks_info)

    # Write test template file
    os.makedirs(test_reports_dir, exist_ok=True)
    template_path = os.path.join(test_reports_dir, f"test-cases-{change_name}.md")
    with open(template_path, "w", encoding="utf-8") as f:
        f.write(template)

    # Inject context telling Claude to review and flesh out the test cases
    context = (
        f"OpenSpec change '{change_name}' is active. "
        f"A test case template has been generated at "
        f"openspec/changes/{change_name}/test-reports/test-cases-{change_name}.md. "
        f"Please review and expand the test cases BEFORE continuing with implementation. "
        f"Fill in specific test scenarios, expected results, and edge cases for each task."
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


def generate_test_template(change_name, change_dir, pending_tasks):
    """Generate a test case template based on pending tasks and proposal."""
    # Read proposal for context
    proposal_summary = ""
    proposal_path = os.path.join(change_dir, "proposal.md")
    if os.path.isfile(proposal_path):
        try:
            with open(proposal_path, "r", encoding="utf-8") as f:
                proposal_summary = f.read().strip()
        except OSError:
            pass

    lines = [
        f"# Test Cases: {change_name}",
        "",
    ]

    if proposal_summary:
        lines.extend([
            "## Proposal Summary",
            "",
            proposal_summary,
            "",
        ])

    lines.extend([
        "## Test Cases by Task",
        "",
    ])

    for i, task in enumerate(pending_tasks, 1):
        # Extract key concepts from task description for test scenarios
        keywords = extract_keywords(task)

        lines.extend([
            f"### Task {i}: {task}",
            "",
            "#### Unit Tests",
        ])

        # Generate unit test placeholders based on keywords
        for j, kw in enumerate(keywords[:3], 1):
            lines.append(f"- [ ] Test {j}: Should correctly handle {kw}")

        lines.extend([
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

    lines.extend([
        "---",
        "",
        "*This template was auto-generated by the OpenSpec PreToolUse hook.*",
        "*Review and expand each test case with specific assertions and expected results.*",
    ])

    return "\n".join(lines)


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
