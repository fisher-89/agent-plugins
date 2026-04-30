#!/usr/bin/env python3
"""
Hook: SessionStart - Suggest git worktree creation for isolated development.

Detects git repository status and provides context about:
- Current branch and uncommitted changes
- Active worktrees
- Recommendation to create a dedicated worktree for feature development
"""

import json
import os
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


def is_in_worktree(cwd):
    """Check if current directory is inside a git worktree (not main worktree)."""
    # git rev-parse --git-common-dir differs from --git-dir in worktrees
    git_dir = run_git(["rev-parse", "--git-dir"], cwd)
    common_dir = run_git(["rev-parse", "--git-common-dir"], cwd)
    if git_dir and common_dir:
        # In main worktree, these are typically the same (or .git vs .git)
        # In linked worktrees, git-dir is .git/worktrees/<name> while common-dir is .git
        return git_dir != common_dir
    return False


def main():
    input_data = json.load(sys.stdin)
    cwd = input_data.get("cwd", "")

    # Check if inside a git repository
    git_dir = run_git(["rev-parse", "--git-dir"], cwd)
    if not git_dir:
        output_result("")
        return

    # Already in a worktree — no need to suggest
    if is_in_worktree(cwd):
        output_result("")
        return

    # Get current branch
    branch = run_git(["branch", "--show-current"], cwd) or "detached HEAD"

    # Get uncommitted changes
    status_output = run_git(["status", "--porcelain"], cwd)
    has_changes = bool(status_output)
    changed_files = []
    if status_output:
        for line in status_output.splitlines():
            if len(line) >= 4:
                changed_files.append(line[3:].strip())
            elif len(line) >= 2:
                changed_files.append(line[2:].strip())

    # Count active worktrees
    worktree_list = run_git(["worktree", "list", "--porcelain"], cwd)
    worktree_count = 0
    if worktree_list:
        worktree_count = worktree_list.count("worktree ")

    # Build context
    lines = ["Git Worktree Status:", ""]

    lines.append(f"- Branch: {branch}")
    lines.append(f"- Worktrees: {worktree_count} active")

    if has_changes:
        lines.append(f"- Uncommitted changes: {len(changed_files)} file(s)")
        # Show up to 10 changed files
        for f in changed_files[:10]:
            lines.append(f"  - {f}")
        if len(changed_files) > 10:
            lines.append(f"  - ... and {len(changed_files) - 10} more")

        lines.append("")
        lines.append(
            "Uncommitted changes are present in the main working tree. "
            "A dedicated git worktree can isolate feature development. "
            "The user may want to carry these changes into the new worktree "
            "or keep them in the current tree. Consider asking the user about "
            "their preference before using the EnterWorktree tool."
        )
    else:
        lines.append("- Working tree: clean")
        lines.append("")
        lines.append(
            "The working tree is clean. A dedicated git worktree can isolate "
            "feature development from the main branch. Consider suggesting the "
            "EnterWorktree tool if the user is starting new feature work."
        )

    output_result("\n".join(lines))


def output_result(additional_context):
    """Output the hook result as JSON."""
    result = {
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": additional_context,
        }
    }
    json.dump(result, sys.stdout)


if __name__ == "__main__":
    main()
