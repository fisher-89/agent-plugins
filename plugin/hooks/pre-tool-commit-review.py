#!/usr/bin/env python3
"""
Hook: PreToolUse - Detect git commit and trigger code review.

When Bash tool is about to execute a git commit command, injects context
to suggest running the code-review agent first.
"""

import json
import os
import re
import subprocess
import sys


def run_git(args, cwd):
    """Run a git command and return stdout, or None on failure."""
    try:
        result = subprocess.run(
            ["git"] + args,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=10,
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except (subprocess.TimeoutExpired, OSError, FileNotFoundError):
        pass
    return None


def main():
    input_data = json.load(sys.stdin)
    tool_name = input_data.get("tool_name", "")
    tool_input = input_data.get("tool_input", {})
    cwd = input_data.get("cwd", "")

    # Only intercept Bash tool calls
    if tool_name != "Bash":
        output_result("allow", "")
        return

    command = tool_input.get("command", "")

    # Check if this is a git commit command
    # Skip if it's --amend (amending existing commit)
    if not is_git_commit_command(command):
        output_result("allow", "")
        return

    # Skip amend operations
    if "--amend" in command:
        output_result("allow", "")
        return

    # Get staged changes
    staged_stat = run_git(["diff", "--cached", "--stat"], cwd)

    if not staged_stat:
        # No staged changes, allow commit to proceed
        output_result("allow", "")
        return

    # Parse staged files count
    staged_files = parse_staged_files(staged_stat)

    if not staged_files:
        output_result("allow", "")
        return

    # Build context for code review
    context = (
        f"Git commit detected with {len(staged_files)} staged file(s). "
        f"Consider running the code-review agent via the Agent tool to review changes "
        f"before committing. Focus areas: logical errors, null/boundary handling, "
        f"redundant logic. "
        f"Staged files: {', '.join(staged_files[:5])}"
        + ("..." if len(staged_files) > 5 else "")
    )

    output_result("allow", context)


def is_git_commit_command(command):
    """Check if the command is a git commit."""
    # Match git commit patterns
    patterns = [
        r"\bgit\s+commit\b",
        r"\bgit-commit\b",
    ]
    for pattern in patterns:
        if re.search(pattern, command):
            return True
    return False


def parse_staged_files(stat_output):
    """Parse staged files from git diff --cached --stat output."""
    files = []
    for line in stat_output.splitlines():
        # Skip the summary line (insertions/deletions)
        if "insertion" in line or "deletion" in line or "|" not in line:
            continue
        # Extract filename (before |)
        parts = line.split("|")
        if parts:
            filename = parts[0].strip()
            if filename:
                files.append(filename)
    return files


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