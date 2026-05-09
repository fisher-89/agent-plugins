#!/usr/bin/env python3
"""
Step report utility for OpenSpec report-driven gates.

Provides save_step_report() to write structured JSON reports for each
SDD workflow step (scope, skeleton, lint, scoped-test, full-test, code-review).
"""

import json
import os
import sys
from datetime import datetime, timezone
from typing import Any, Dict, Optional

REPORT_SCHEMA = "openspec-step-report/v1"


def save_step_report(
    change: str,
    task_id: str,
    step: str,
    status: str,
    details: Dict[str, Any],
    project_root: str = ".",
    duration_ms: Optional[int] = None,
) -> str:
    """
    Save a step report to the reports directory.

    Args:
        change: OpenSpec change name.
        task_id: Task identifier (e.g., "3") or "global" for non-task steps.
        step: Step name (scope, skeleton, lint, scoped-test, full-test, code-review).
        status: Step result (pass, fail, skip).
        details: Step-specific details dict.
        project_root: Path to the project root directory.
        duration_ms: Optional duration in milliseconds.

    Returns:
        Path to the written report file.
    """
    reports_dir = os.path.join(
        project_root, "openspec", "changes", change, "reports"
    )
    os.makedirs(reports_dir, exist_ok=True)

    report = {
        "schema": REPORT_SCHEMA,
        "change": change,
        "task_id": task_id,
        "step": step,
        "status": status,
        "timestamp": datetime.now(timezone.utc).isoformat(),
    }

    if duration_ms is not None:
        report["duration_ms"] = duration_ms

    report["details"] = details

    # Determine filename
    if task_id and task_id != "global":
        filename = f"task-{task_id}_{step}.json"
    else:
        filename = f"{step}.json"

    path = os.path.join(reports_dir, filename)

    with open(path, "w", encoding="utf-8") as f:
        json.dump(report, f, indent=2)

    return path


def load_step_report(
    change: str,
    task_id: str,
    step: str,
    project_root: str = ".",
) -> Optional[Dict[str, Any]]:
    """
    Load a step report from the reports directory.

    Args:
        change: OpenSpec change name.
        task_id: Task identifier or "global".
        step: Step name.
        project_root: Path to the project root directory.

    Returns:
        Parsed report dict, or None if not found.
    """
    if task_id and task_id != "global":
        filename = f"task-{task_id}_{step}.json"
    else:
        filename = f"{step}.json"

    path = os.path.join(
        project_root, "openspec", "changes", change, "reports", filename
    )

    if not os.path.isfile(path):
        return None

    try:
        with open(path, "r", encoding="utf-8") as f:
            return json.load(f)
    except (json.JSONDecodeError, OSError):
        return None


# CLI interface
if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Save or load step reports")
    parser.add_argument("--save", action="store_true", help="Save a report")
    parser.add_argument("--load", action="store_true", help="Load a report")
    parser.add_argument("--change", required=True, help="Change name")
    parser.add_argument("--task-id", default="global", help="Task ID (default: global)")
    parser.add_argument("--step", required=True, help="Step name")
    parser.add_argument("--status", default="pass", help="Status (pass/fail/skip)")
    parser.add_argument("--project-root", default=".", help="Project root")
    parser.add_argument("--details", default="{}", help="JSON details string")
    parser.add_argument("--duration-ms", type=int, help="Duration in milliseconds")

    args = parser.parse_args()

    if args.save:
        try:
            details = json.loads(args.details)
        except json.JSONDecodeError:
            details = {"raw": args.details}

        path = save_step_report(
            change=args.change,
            task_id=args.task_id,
            step=args.step,
            status=args.status,
            details=details,
            project_root=args.project_root,
            duration_ms=args.duration_ms,
        )
        print(f"Saved: {path}")

    elif args.load:
        report = load_step_report(
            change=args.change,
            task_id=args.task_id,
            step=args.step,
            project_root=args.project_root,
        )
        if report:
            print(json.dumps(report, indent=2))
        else:
            print("Report not found.", file=sys.stderr)
            sys.exit(1)

    else:
        parser.print_help()
        sys.exit(1)
