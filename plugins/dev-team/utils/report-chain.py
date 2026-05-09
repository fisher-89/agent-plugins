#!/usr/bin/env python3
"""
Report chain checker for OpenSpec report-driven gates.

Validates that SDD workflow steps have been executed in order by checking
step report files in the reports/ directory.
"""

import json
import os
import re
from typing import List, Tuple, Optional


def check_report_chain(change_dir: str, task_id: str) -> Tuple[bool, Optional[str]]:
    """
    Check if report chain is complete for a task.

    Required steps: scope, lint, scoped-test (skeleton is optional)

    Args:
        change_dir: Path to the change directory.
        task_id: Task identifier.

    Returns:
        Tuple of (ok, error_message). If ok=True, error_message is None.
    """
    reports_dir = os.path.join(change_dir, "reports")
    if not os.path.isdir(reports_dir):
        return False, "No reports directory found"

    # Required steps (skeleton is optional as some tasks don't need new tests)
    required_steps = ["scope", "lint", "scoped-test"]
    reports = []

    for step in required_steps:
        path = os.path.join(reports_dir, f"task-{task_id}_{step}.json")
        if not os.path.isfile(path):
            return False, f"Missing report: {step}"
        try:
            with open(path, "r", encoding="utf-8") as f:
                report = json.load(f)
            reports.append(report)
            if report.get("status") != "pass":
                return False, f"Step '{step}' failed"
        except (json.JSONDecodeError, OSError):
            return False, f"Invalid report: {step}"

    # Check timestamp ordering (should be strictly increasing)
    timestamps = [r["timestamp"] for r in reports]
    if timestamps != sorted(timestamps):
        return False, "Report timestamps out of order"

    return True, None


def check_all_report_chains(
    change_dir: str,
    tasks_md_path: str
) -> Tuple[bool, List[str]]:
    """
    Check report chains for all tasks.

    Args:
        change_dir: Path to the change directory.
        tasks_md_path: Path to tasks.md file.

    Returns:
        Tuple of (ok, issues_list). If ok=True, issues_list is empty.
    """
    issues = []
    tasks = parse_tasks(tasks_md_path)

    for task_id, task_desc, completed in tasks:
        if not completed:
            continue  # Skip incomplete tasks
        ok, err = check_report_chain(change_dir, task_id)
        if not ok:
            issues.append(f"Task {task_id}: {err}")

    return len(issues) == 0, issues


def check_final_reports(change_dir: str) -> Tuple[bool, Optional[str]]:
    """
    Check full-test and code-review reports exist and are ordered.

    Args:
        change_dir: Path to the change directory.

    Returns:
        Tuple of (ok, error_message).
    """
    reports_dir = os.path.join(change_dir, "reports")

    full_test_path = os.path.join(reports_dir, "full-test.json")
    review_path = os.path.join(reports_dir, "code-review.json")

    if not os.path.isfile(full_test_path):
        return False, "Missing full-test.json"
    if not os.path.isfile(review_path):
        return False, "Missing code-review.json"

    try:
        with open(full_test_path, "r", encoding="utf-8") as f:
            full_test = json.load(f)
        with open(review_path, "r", encoding="utf-8") as f:
            review = json.load(f)

        if full_test["timestamp"] >= review["timestamp"]:
            return False, "full-test must run before code-review"

        if full_test.get("status") != "pass":
            return False, "full-test did not pass"

        if review.get("status") == "fail":
            return False, "code-review has errors"

    except (json.JSONDecodeError, OSError, KeyError) as e:
        return False, f"Invalid report file: {e}"

    return True, None


def get_current_task(change_dir: str) -> Optional[str]:
    """
    Get the current incomplete task ID from tasks.md.

    Args:
        change_dir: Path to the change directory.

    Returns:
        Task ID string or None if all complete.
    """
    tasks_md_path = os.path.join(change_dir, "tasks.md")
    if not os.path.isfile(tasks_md_path):
        return None

    tasks = parse_tasks(tasks_md_path)
    for task_id, task_desc, completed in tasks:
        if not completed:
            return task_id

    return None


def parse_tasks(tasks_md_path: str) -> List[Tuple[str, str, bool]]:
    """
    Parse tasks.md to extract task IDs, descriptions, and completion status.

    Args:
        tasks_md_path: Path to tasks.md file.

    Returns:
        List of tuples: (task_id, description, completed).
    """
    tasks = []
    task_id = 0

    try:
        with open(tasks_md_path, "r", encoding="utf-8") as f:
            for line in f:
                match = re.match(r"^\s*- \[([ x])\]\s*(.+)$", line)
                if match:
                    task_id += 1
                    completed = match.group(1) == "x"
                    description = match.group(2).strip()
                    tasks.append((str(task_id), description, completed))
    except OSError:
        pass

    return tasks


# CLI interface
if __name__ == "__main__":
    import argparse
    import sys

    parser = argparse.ArgumentParser(description="Check report chains")
    parser.add_argument("--change-dir", required=True, help="Path to change directory")
    parser.add_argument("--task-id", help="Specific task ID to check")
    parser.add_argument("--all", action="store_true", help="Check all tasks")
    parser.add_argument("--final", action="store_true", help="Check final reports")
    parser.add_argument("--current", action="store_true", help="Get current task")

    args = parser.parse_args()

    if args.current:
        task_id = get_current_task(args.change_dir)
        if task_id:
            print(task_id)
        else:
            print("All tasks complete or no tasks found.")
        sys.exit(0)

    if args.final:
        ok, err = check_final_reports(args.change_dir)
        if ok:
            print("Final reports OK.")
        else:
            print(f"FAIL: {err}", file=sys.stderr)
        sys.exit(0 if ok else 1)

    if args.all:
        tasks_md = os.path.join(args.change_dir, "tasks.md")
        ok, issues = check_all_report_chains(args.change_dir, tasks_md)
        if ok:
            print("All report chains OK.")
        else:
            print("FAIL:", file=sys.stderr)
            for issue in issues:
                print(f"  - {issue}", file=sys.stderr)
        sys.exit(0 if ok else 1)

    if args.task_id:
        ok, err = check_report_chain(args.change_dir, args.task_id)
        if ok:
            print(f"Task {args.task_id} report chain OK.")
        else:
            print(f"FAIL: {err}", file=sys.stderr)
        sys.exit(0 if ok else 1)

    parser.print_help()
    sys.exit(1)
