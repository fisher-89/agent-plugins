#!/usr/bin/env python3
"""
Hook: SessionStart - Check for uncommitted changes and suggest worktree creation.

When starting a session with uncommitted changes in the main working tree,
this hook outputs context suggesting the user may want to create a worktree
to isolate their work.
"""

import json
import os
import subprocess
import sys


def get_git_status():
    """Get git status info: current branch, uncommitted files, worktree status."""
    result = {
        "branch": None,
        "has_uncommitted": False,
        "uncommitted_files": [],
        "is_worktree": False,
    }

    try:
        # Check if in a git repo
        subprocess.run(
            ["git", "rev-parse", "--git-dir"],
            capture_output=True,
            check=True,
            shell=True,
        )

        # Get current branch
        branch_result = subprocess.run(
            ["git", "branch", "--show-current"],
            capture_output=True,
            text=True,
            shell=True,
        )
        result["branch"] = branch_result.stdout.strip() or "HEAD"

        # Check if in a worktree (worktrees have .git as a file, not a directory)
        git_dir = subprocess.run(
            ["git", "rev-parse", "--git-dir"],
            capture_output=True,
            text=True,
            shell=True,
        ).stdout.strip()

        # If .git path contains "worktrees", we're in a worktree
        result["is_worktree"] = "worktrees" in git_dir

        # Get uncommitted files
        status_result = subprocess.run(
            ["git", "status", "--porcelain"],
            capture_output=True,
            text=True,
            shell=True,
        )

        if status_result.stdout.strip():
            result["has_uncommitted"] = True
            # Parse porcelain output
            for line in status_result.stdout.strip().split("\n"):
                if line:
                    # Format: XY filename (XY is status code)
                    filename = line[3:] if line[1] == " " else line[2:]
                    result["uncommitted_files"].append(filename)

    except subprocess.CalledProcessError:
        # Not a git repo
        pass

    return result


def output_result(additional_context):
    """Output the hook result as JSON."""
    result = {
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": additional_context,
        }
    }
    json.dump(result, sys.stdout)


def main():
    status = get_git_status()

    # Only suggest worktree if:
    # 1. In main working tree (not already in a worktree)
    # 2. Have uncommitted changes
    if status["is_worktree"] or not status["has_uncommitted"]:
        output_result("")
        return

    # Build context message
    file_count = len(status["uncommitted_files"])
    files_preview = status["uncommitted_files"][:3]
    files_str = ", ".join(files_preview)
    if file_count > 3:
        files_str += f" (+{file_count - 3} more)"

    context = (
        f"Git Worktree Status:\n\n"
        f"- Branch: {status['branch']}\n"
        f"- Worktrees: 1 active\n"
        f"- Uncommitted changes: {file_count} file(s)\n"
        f"  - {files_str}\n\n"
        f"Uncommitted changes are present in the main working tree. "
        f"A dedicated git worktree can isolate feature development. "
        f"Consider asking the user if they want to create a worktree for isolated work."
    )

    output_result(context)


if __name__ == "__main__":
    main()
