#!/usr/bin/env python3
"""
Eval check script — validates that all PGE phases have passed and all tasks are complete.

Reads eval.json, extracts the latest entry per phase by timestamp, verifies:
- All required phases (P1-P5, P7, P9) have verdict "pass"
- No gaps in the phase sequence
- All tasks in tasks.md are marked complete ([x])

Exit 0 on pass, non-zero on fail.
"""

import json
import os
import sys
from pathlib import Path

REQUIRED_PHASES = [
    "01-requirements",
    "02-test-design",
    "03-dev-proposal",
    "04-test-gen",
    "05-implementation",
    "06-code-review",
    "07-acceptance",
]


def find_change_dir(project_root):
    """Find the active change directory."""
    changes_dir = os.path.join(project_root, "openspec", "changes")
    if not os.path.isdir(changes_dir):
        return None

    # Use active-change.py if available (same directory)
    try:
        sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
        from importlib import util
        ac_path = os.path.join(os.path.dirname(__file__), "active-change.py")
        if os.path.isfile(ac_path):
            spec = util.spec_from_file_location("active_change", ac_path)
            module = util.module_from_spec(spec)
            spec.loader.exec_module(module)
            result = module.find_active_change(changes_dir, project_root)
            if result:
                return result[1]  # change_dir
    except Exception:
        pass

    # Fallback: find first change with phases/eval.json or tasks.md
    try:
        for entry in os.listdir(changes_dir):
            entry_path = os.path.join(changes_dir, entry)
            if not os.path.isdir(entry_path) or entry == "archive":
                continue
            phases_dir = os.path.join(entry_path, "phases")
            if os.path.isdir(phases_dir):
                return entry_path
            if os.path.isfile(os.path.join(entry_path, "tasks.md")):
                return entry_path
    except OSError:
        pass

    return None


def read_eval_json(change_dir):
    """Read eval.json from the change's phases directory."""
    eval_path = os.path.join(change_dir, "phases", "eval.json")
    if not os.path.isfile(eval_path):
        return None
    try:
        with open(eval_path, "r", encoding="utf-8") as f:
            return json.load(f)
    except (json.JSONDecodeError, OSError) as e:
        print(f"ERROR: Failed to parse eval.json: {e}")
        return None


def get_latest_per_phase(eval_entries):
    """Extract the latest entry per phase by timestamp."""
    if not isinstance(eval_entries, list):
        return {}

    phases = {}
    for entry in eval_entries:
        phase = entry.get("phase", "")
        ts = entry.get("timestamp", "")
        if phase not in phases or ts > phases[phase]["timestamp"]:
            phases[phase] = entry
    return phases


def count_tasks(tasks_path):
    """Count total and completed tasks in tasks.md."""
    import re
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


def validate_eval_chain(change_dir):
    """Validate the eval chain. Returns (passed, message)."""
    eval_entries = read_eval_json(change_dir)
    if eval_entries is None:
        return False, "eval.json not found or empty — no phases have been evaluated"

    latest = get_latest_per_phase(eval_entries)
    errors = []

    # Check: all required phases present and passed
    for phase in REQUIRED_PHASES:
        if phase not in latest:
            errors.append(f"MISSING: Phase '{phase}' has no entries in eval.json")
            continue
        entry = latest[phase]
        if entry.get("verdict") != "pass":
            failing_items = [
                item["item_id"]
                for item in entry.get("items", [])
                if not item.get("pass", False)
            ]
            errors.append(
                f"FAILED: Phase '{phase}' verdict is '{entry.get('verdict')}' "
                f"(attempt {entry.get('attempt', '?')}). "
                f"Failing items: {failing_items}"
            )

    # Check: no active backtrack markers in latest entries
    for phase, entry in latest.items():
        backtrack = entry.get("backtrack_to")
        if backtrack:
            errors.append(
                f"BACKTRACK: Phase '{phase}' has backtrack_to='{backtrack}' — resolve before archive"
            )

    if errors:
        return False, "Eval chain validation failed:\n  - " + "\n  - ".join(errors)

    return True, "All required phases passed"


def validate_tasks(change_dir):
    """Validate all tasks in tasks.md are complete. Returns (passed, message)."""
    tasks_path = os.path.join(change_dir, "tasks.md")
    if not os.path.isfile(tasks_path):
        # No tasks.md — check phases/tasks.md
        tasks_path = os.path.join(change_dir, "phases", "tasks.md")
        if not os.path.isfile(tasks_path):
            return False, "tasks.md not found"

    total, done = count_tasks(tasks_path)
    if total == 0:
        return True, "No tasks defined (empty tasks.md)"

    if done < total:
        incomplete = total - done
        return False, f"Tasks incomplete: {done}/{total} complete ({incomplete} remaining)"

    return True, f"All {total} tasks complete"


def main():
    import argparse

    parser = argparse.ArgumentParser(
        description="Validate PGE eval chain and task completion"
    )
    parser.add_argument(
        "--change", type=str, help="Change name (optional, auto-detected if omitted)"
    )
    parser.add_argument(
        "--project-root", type=str, default=".", help="Project root directory"
    )
    parser.add_argument(
        "--json", action="store_true", help="Output result as JSON"
    )
    args = parser.parse_args()

    project_root = os.path.abspath(args.project_root)

    if args.change:
        change_dir = os.path.join(
            project_root, "openspec", "changes", args.change
        )
    else:
        change_dir = find_change_dir(project_root)

    if not change_dir or not os.path.isdir(change_dir):
        msg = "No active change found"
        if args.json:
            print(json.dumps({"passed": False, "error": msg}))
        else:
            print(f"FAIL: {msg}")
        sys.exit(1)

    change_name = os.path.basename(change_dir)

    eval_ok, eval_msg = validate_eval_chain(change_dir)
    tasks_ok, tasks_msg = validate_tasks(change_dir)

    all_ok = eval_ok and tasks_ok

    if args.json:
        print(json.dumps({
            "passed": all_ok,
            "change": change_name,
            "eval_check": {"passed": eval_ok, "message": eval_msg},
            "tasks_check": {"passed": tasks_ok, "message": tasks_msg},
        }))
    else:
        if all_ok:
            print(f"PASS: All checks passed for '{change_name}'")
            print(f"  Eval: {eval_msg}")
            print(f"  Tasks: {tasks_msg}")
        else:
            print(f"FAIL: Checks failed for '{change_name}'")
            if not eval_ok:
                print(f"  Eval: {eval_msg}")
            if not tasks_ok:
                print(f"  Tasks: {tasks_msg}")

    sys.exit(0 if all_ok else 1)


if __name__ == "__main__":
    main()
