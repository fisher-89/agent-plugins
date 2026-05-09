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

# Import shared hook output utility
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PLUGIN_ROOT = os.path.dirname(SCRIPT_DIR)
UTILS_DIR = os.path.join(PLUGIN_ROOT, "utils")

if UTILS_DIR not in sys.path:
    sys.path.insert(0, UTILS_DIR)

from hook_output import output_session_start


def get_git_status():
    """
    Get git status info: current branch, uncommitted files, worktree status.

    Returns tuple: (result_dict, error_message or None)
    """
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

        return result, None

    except subprocess.CalledProcessError as e:
        # Not a git repo or git command failed
        error_msg = f"Git command failed: {e}"
        if e.stderr:
            error_msg += f" (stderr: {e.stderr.decode() if isinstance(e.stderr, bytes) else e.stderr})"
        return result, error_msg
    except Exception as e:
        # Unexpected error
        return result, f"Unexpected error checking git status: {e}"


def main():
    status, error = get_git_status()

    # If there was an error checking git status, output the error
    if error:
        output_session_start(f"Session worktree hook error: {error}")
        return

    # Only suggest worktree if:
    # 1. In main working tree (not already in a worktree)
    # 2. Have uncommitted changes
    if status["is_worktree"] or not status["has_uncommitted"]:
        output_session_start()
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

    output_session_start(context)


if __name__ == "__main__":
    main()
