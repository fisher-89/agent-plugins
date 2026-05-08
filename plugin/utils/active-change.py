#!/usr/bin/env python3
"""
Shared utilities for finding active OpenSpec changes.

This module provides a common implementation for finding the active change
with priority ordering:
1. Change with pending tasks (highest priority)
2. Change matching staged files (inferred from git)
3. Most recently modified change (fallback)
"""

import os
import re
import subprocess


def find_active_change(changes_dir, cwd=""):
    """Find the active change with priority ordering.

    Priority:
    1. Change with pending tasks (original behavior)
    2. Change matching staged files (inferred from git)
    3. Most recently modified change (fallback)

    Args:
        changes_dir: Path to the openspec/changes directory
        cwd: Current working directory (for git commands)

    Returns:
        Tuple (change_name, change_path) or None if no changes found
    """
    changes = []

    try:
        for entry in os.listdir(changes_dir):
            entry_path = os.path.join(changes_dir, entry)
            if not os.path.isdir(entry_path) or entry == "archive":
                continue

            tasks_path = os.path.join(entry_path, "tasks.md")
            has_pending = False
            total, done = 0, 0
            if os.path.isfile(tasks_path):
                total, done = count_tasks(tasks_path)
                has_pending = done < total

            mtime = get_latest_mtime(entry_path)
            changes.append({
                "name": entry,
                "path": entry_path,
                "has_pending": has_pending,
                "total": total,
                "done": done,
                "mtime": mtime,
            })
    except OSError:
        return None

    if not changes:
        return None

    # Priority 1: Pending tasks
    pending = [c for c in changes if c["has_pending"]]
    if len(pending) == 1:
        return (pending[0]["name"], pending[0]["path"])
    if len(pending) > 1:
        # Multiple pending — try git inference
        staged_match = match_staged_to_change(cwd, pending)
        if staged_match:
            return (staged_match["name"], staged_match["path"])
        # Fallback: most recently modified pending
        pending.sort(key=lambda c: c["mtime"], reverse=True)
        return (pending[0]["name"], pending[0]["path"])

    # Priority 2: All tasks done — infer from staged files
    staged_match = match_staged_to_change(cwd, changes)
    if staged_match:
        return (staged_match["name"], staged_match["path"])

    # Priority 3: Most recently modified
    changes.sort(key=lambda c: c["mtime"], reverse=True)
    return (changes[0]["name"], changes[0]["path"])


def count_tasks(tasks_path):
    """Count total and completed tasks in a tasks.md file.

    Args:
        tasks_path: Path to the tasks.md file

    Returns:
        Tuple (total, done) counts
    """
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


def match_staged_to_change(cwd, changes):
    """Match staged git files to a change directory.

    Args:
        cwd: Current working directory (for git commands)
        changes: List of change dicts with "path" key

    Returns:
        Matched change dict or None
    """
    staged_files = get_staged_files(cwd)
    if not staged_files:
        return None

    for change in changes:
        change_dir = change["path"]
        for staged in staged_files:
            # Check if staged file is under change directory
            staged_abs = os.path.normpath(os.path.join(cwd, staged))
            change_abs = os.path.normpath(change_dir)
            if staged_abs.startswith(change_abs):
                return change

    # Also check: if there's only one change, it's likely the one
    if len(changes) == 1:
        return changes[0]

    return None


def get_latest_mtime(directory):
    """Get the most recent modification time in a directory tree.

    Args:
        directory: Path to the directory

    Returns:
        Modification timestamp (float) or 0 if no files
    """
    latest = 0
    try:
        for root, dirs, files in os.walk(directory):
            for f in files:
                try:
                    mtime = os.path.getmtime(os.path.join(root, f))
                    latest = max(latest, mtime)
                except OSError:
                    pass
    except OSError:
        pass
    return latest


def get_staged_files(cwd):
    """Get list of staged files from git.

    Args:
        cwd: Current working directory (for git commands)

    Returns:
        List of staged file paths (relative to cwd)
    """
    try:
        result = subprocess.run(
            ["git", "diff", "--cached", "--name-only"],
            cwd=cwd, capture_output=True, text=True, timeout=10, shell=True
        )
        if result.returncode == 0:
            return [f.strip() for f in result.stdout.splitlines() if f.strip()]
    except (subprocess.TimeoutExpired, OSError, FileNotFoundError):
        pass
    return []


if __name__ == "__main__":
    # Simple test
    import sys
    import json

    if len(sys.argv) < 2:
        print("Usage: active-change.py <changes_dir> [cwd]")
        sys.exit(1)

    changes_dir = sys.argv[1]
    cwd = sys.argv[2] if len(sys.argv) > 2 else ""

    result = find_active_change(changes_dir, cwd)
    if result:
        name, path = result
        total, done = 0, 0
        tasks_path = os.path.join(path, "tasks.md")
        if os.path.isfile(tasks_path):
            total, done = count_tasks(tasks_path)
        print(json.dumps({
            "name": name,
            "path": path,
            "tasks": {"total": total, "done": done}
        }, indent=2))
    else:
        print(json.dumps({"error": "No active change found"}, indent=2))
