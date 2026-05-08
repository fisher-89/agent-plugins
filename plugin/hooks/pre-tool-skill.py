#!/usr/bin/env python3
"""
Hook: PreToolUse (Skill) - Intercept Skill calls for OpenSpec workflow gates.

Intercepts Skill tool calls:
1. archive-change: Run compliance check, deny on failure
2. apply-change: Check review loop state, warn on paused
"""

import json
import os
import subprocess
import sys

# Import shared utilities
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PLUGIN_ROOT = os.path.dirname(SCRIPT_DIR)
UTILS_DIR = os.path.join(PLUGIN_ROOT, "utils")

# Add utils to path for import
if UTILS_DIR not in sys.path:
    sys.path.insert(0, UTILS_DIR)

try:
    import importlib.util
    _ac_path = os.path.join(UTILS_DIR, "active-change.py")
    if os.path.isfile(_ac_path):
        _ac_spec = importlib.util.spec_from_file_location("active_change", _ac_path)
        _ac_module = importlib.util.module_from_spec(_ac_spec)
        _ac_spec.loader.exec_module(_ac_module)
        find_active_change = _ac_module.find_active_change
    else:
        def find_active_change(changes_dir, cwd=""):
            return None
except Exception:
    def find_active_change(changes_dir, cwd=""):
        return None

try:
    _rc_path = os.path.join(UTILS_DIR, "report-chain.py")
    if os.path.isfile(_rc_path):
        _rc_spec = importlib.util.spec_from_file_location("report_chain", _rc_path)
        _rc_module = importlib.util.module_from_spec(_rc_spec)
        _rc_spec.loader.exec_module(_rc_module)
        check_all_report_chains = _rc_module.check_all_report_chains
        check_final_reports = _rc_module.check_final_reports
        HAS_REPORT_CHAIN = True
    else:
        HAS_REPORT_CHAIN = False
except Exception:
    HAS_REPORT_CHAIN = False


def main():
    input_data = json.load(sys.stdin)
    tool_name = input_data.get("tool_name", "")
    tool_input = input_data.get("tool_input", {})
    cwd = input_data.get("cwd", "")

    # Only intercept Skill tool calls
    if tool_name != "Skill":
        output_result("allow", "")
        return

    skill_name = tool_input.get("skill", "")

    # Route by skill
    if "archive" in skill_name:
        handle_archive_skill(cwd)
    elif "apply" in skill_name:
        handle_apply_skill(cwd)
    else:
        output_result("allow", "")


def handle_archive_skill(cwd):
    """Force compliance check and report chain check before archive."""
    changes_dir = os.path.join(cwd, "openspec", "changes")

    if not os.path.isdir(changes_dir):
        output_result("allow", "")
        return

    # Find active change
    active_change = find_active_change(changes_dir, cwd)
    if not active_change:
        output_result("allow", "")
        return

    change_name, change_dir = active_change

    # Run compliance check (if available)
    compliance_check_path = os.path.join(UTILS_DIR, "compliance-check.py")

    if not os.path.isfile(compliance_check_path):
        # Compliance checker not available — allow with warning
        output_result(
            "allow",
            f"Compliance checker not available. Proceeding with archive of '{change_name}'."
        )
        return

    try:
        result = subprocess.run(
            ["python", compliance_check_path,
             "--change", change_name, "--project-root", cwd, "--json"],
            capture_output=True, text=True, timeout=30, shell=True
        )

        if result.returncode != 0:
            # Compliance check failed to run — allow with warning
            output_result(
                "allow",
                f"Compliance check failed to run for '{change_name}'. Proceeding with archive."
            )
            return

        data = json.loads(result.stdout)

        if not data.get("passed", False):
            blocking = data.get("blocking_issues", [])
            issues = format_blocking_issues(blocking)
            context = (
                f"ARCHIVE BLOCKED: {len(blocking)} compliance issue(s) for '{change_name}'.\n"
                f"{issues}\n"
                f"Fix these issues before archiving, or explicitly override."
            )
            output_result("deny", context)
            return

    except subprocess.TimeoutExpired:
        output_result(
            "allow",
            f"Compliance check timed out for '{change_name}'. Proceeding with archive."
        )
        return
    except (json.JSONDecodeError, OSError) as e:
        output_result(
            "allow",
            f"Compliance check error for '{change_name}': {e}. Proceeding with archive."
        )
        return

    # === Report Chain Check ===
    if HAS_REPORT_CHAIN:
        tasks_md = os.path.join(change_dir, "tasks.md")
        all_ok, issues = check_all_report_chains(change_dir, tasks_md)
        if not all_ok:
            context = (
                f"ARCHIVE BLOCKED: Report chain incomplete.\n"
                f"Issues:\n" + "\n".join(f"  - {i}" for i in issues) + "\n"
                f"Ensure all tasks have complete report chains before archiving."
            )
            output_result("deny", context)
            return

        # Check final reports
        ok, err = check_final_reports(change_dir)
        if not ok:
            output_result("deny", f"ARCHIVE BLOCKED: {err}")
            return

    output_result("allow", f"Compliance check and report chains passed for '{change_name}'.")


def handle_apply_skill(cwd):
    """Check review loop state before apply."""
    changes_dir = os.path.join(cwd, "openspec", "changes")

    if not os.path.isdir(changes_dir):
        output_result("allow", "")
        return

    active_change = find_active_change(changes_dir, cwd)

    if not active_change:
        output_result("allow", "")
        return

    change_name, change_dir = active_change

    # Check review loop state
    state_path = os.path.join(change_dir, "review-loop-state.json")
    if os.path.isfile(state_path):
        try:
            with open(state_path, "r", encoding="utf-8") as f:
                state = json.load(f)

            status = state.get("status", "idle")
            loop_count = state.get("loop_count", 0)
            max_loops = state.get("max_loops", 3)
            current_errors = state.get("current_errors", 0)

            if status == "paused":
                context = (
                    f"Review loop PAUSED for '{change_name}'. "
                    f"Loop {loop_count}/{max_loops} reached. "
                    f"Manual intervention required."
                )
                output_result("allow", context)
                return

            if status == "running" and current_errors > 0:
                context = (
                    f"Review loop active for '{change_name}': "
                    f"Loop {loop_count}/{max_loops}, {current_errors} error(s) remaining. "
                    f"Fix tasks have been appended to tasks.md. "
                    f"Implement the fix tasks before proceeding."
                )
                output_result("allow", context)
                return

        except (json.JSONDecodeError, OSError):
            pass

    output_result("allow", "")


def format_blocking_issues(issues):
    """Format blocking issues for display."""
    if not issues:
        return "No issues listed."

    lines = []
    for issue in issues[:5]:  # Show max 5 issues
        check_id = issue.get("check", "Unknown")
        message = issue.get("message", "No message")
        lines.append(f"  - [{check_id}] {message}")

    if len(issues) > 5:
        lines.append(f"  ... (+{len(issues) - 5} more)")

    return "\n".join(lines)


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
