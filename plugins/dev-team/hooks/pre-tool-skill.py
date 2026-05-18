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

if UTILS_DIR not in sys.path:
    sys.path.insert(0, UTILS_DIR)

from hook_output import output_pre_tool_use

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



def main():
    input_data = json.load(sys.stdin)
    tool_name = input_data.get("tool_name", "")
    tool_input = input_data.get("tool_input", {})
    cwd = input_data.get("cwd", "")

    # Only intercept Skill tool calls
    if tool_name != "Skill":
        output_pre_tool_use("allow", "")
        return

    skill_name = tool_input.get("skill", "")

    # Route by skill
    if "archive" in skill_name:
        handle_archive_skill(cwd)
    elif "apply" in skill_name:
        handle_apply_skill(cwd)
    elif "phase-" in skill_name:
        handle_phase_skill(cwd, skill_name)
    else:
        output_pre_tool_use("allow", "")


def handle_archive_skill(cwd):
    """Run eval check before archive (archive flow step 1 of 3)."""
    changes_dir = os.path.join(cwd, "openspec", "changes")

    if not os.path.isdir(changes_dir):
        output_pre_tool_use("allow", "")
        return

    # Find active change
    active_change = find_active_change(changes_dir, cwd)
    if not active_change:
        output_pre_tool_use("allow", "")
        return

    change_name, change_dir = active_change

    # Run eval check script
    eval_check_path = os.path.join(UTILS_DIR, "eval-check.py")

    if not os.path.isfile(eval_check_path):
        output_pre_tool_use(
            "allow",
            f"Eval check script not available. Proceeding with archive of '{change_name}'."
        )
        return

    try:
        result = subprocess.run(
            ["python", eval_check_path,
             "--change", change_name, "--project-root", cwd, "--json"],
            capture_output=True, text=True, timeout=30, shell=True
        )

        if result.returncode != 0:
            try:
                data = json.loads(result.stdout)
                msg = data.get("error", result.stdout)
            except json.JSONDecodeError:
                msg = result.stdout or result.stderr
            context = (
                f"ARCHIVE BLOCKED: Eval check failed for '{change_name}'.\n"
                f"{msg}\n"
                f"Complete all PGE phases and tasks before archiving."
            )
            output_pre_tool_use("deny", context)
            return

        # Eval check passed
        output_pre_tool_use(
            "allow",
            f"Eval check passed for '{change_name}'. Archive will proceed."
        )

    except subprocess.TimeoutExpired:
        output_pre_tool_use(
            "allow",
            f"Eval check timed out for '{change_name}'. Proceeding with archive."
        )
    except (json.JSONDecodeError, OSError) as e:
        output_pre_tool_use(
            "allow",
            f"Eval check error for '{change_name}': {e}. Proceeding with archive."
        )


def handle_apply_skill(cwd):
    """Check eval state before apply (apply-change is now phase-implement)."""
    changes_dir = os.path.join(cwd, "openspec", "changes")

    if not os.path.isdir(changes_dir):
        output_pre_tool_use("allow", "")
        return

    active_change = find_active_change(changes_dir, cwd)

    if not active_change:
        output_pre_tool_use("allow", "")
        return

    change_name, change_dir = active_change

    # Check for existing eval progress
    eval_json_path = os.path.join(change_dir, "phases", "eval.json")
    if os.path.isfile(eval_json_path):
        try:
            with open(eval_json_path, "r", encoding="utf-8") as f:
                eval_entries = json.load(f)

            # Find latest phase completions
            latest = {}
            for entry in eval_entries:
                phase = entry.get("phase", "")
                ts = entry.get("timestamp", "")
                if phase not in latest or ts > latest[phase]["timestamp"]:
                    latest[phase] = entry

            # Surface eval state to user
            completed = [p for p, e in latest.items() if e.get("verdict") == "pass"]
            if completed:
                context = (
                    f"Eval state for '{change_name}': "
                    f"Completed phases: {', '.join(sorted(completed))}. "
                    f"Pending phases will be evaluated after implementation."
                )
                output_pre_tool_use("allow", context)
                return
        except (json.JSONDecodeError, OSError):
            pass

    output_pre_tool_use("allow", "")


def handle_phase_skill(cwd, skill_name):
    """Route phase skill invocations and provide eval state context."""
    changes_dir = os.path.join(cwd, "openspec", "changes")

    if not os.path.isdir(changes_dir):
        output_pre_tool_use("allow", "")
        return

    active_change = find_active_change(changes_dir, cwd)
    if not active_change:
        output_pre_tool_use("allow", "")
        return

    change_name, change_dir = active_change

    # Check eval.json for backtrack markers and current state
    eval_json_path = os.path.join(change_dir, "phases", "eval.json")
    backtrack_warning = ""
    if os.path.isfile(eval_json_path):
        try:
            with open(eval_json_path, "r", encoding="utf-8") as f:
                eval_entries = json.load(f)

            for entry in eval_entries:
                bt = entry.get("backtrack_to")
                if bt:
                    backtrack_warning = (
                        f"BACKTRACK: Phase '{entry.get('phase')}' requests "
                        f"re-evaluation of '{bt}'. Handle backtrack before proceeding."
                    )
                    break
        except (json.JSONDecodeError, OSError):
            pass

    # EVALUATOR-ONLY routing (no Generator for code-review, acceptance)
    if "code-review" in skill_name or "acceptance" in skill_name:
        context = (
            f"EVALUATOR-ONLY phase: '{skill_name}' will invoke only the Evaluator, "
            f"no Planner or Generator."
        )
        if backtrack_warning:
            context += f"\n{backtrack_warning}"
        output_pre_tool_use("allow", context)
        return

    # DESIGN and EXECUTION phases
    if backtrack_warning:
        output_pre_tool_use("allow", backtrack_warning)
    else:
        output_pre_tool_use("allow", "")




if __name__ == "__main__":
    main()
