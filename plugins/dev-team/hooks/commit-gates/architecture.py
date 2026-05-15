#!/usr/bin/env python3
"""
Hook: PreToolUse (Bash) - Architecture validation gate for git commit.

Intercepts Bash tool calls that contain git commit commands.
Checks that staged code changes have a corresponding architecture validation
report (architecture-validate-*.json) in openspec/changes/&lt;name&gt;/reports/.

Behavior:
- If no architecture model exists: allow (nothing to validate against)
- If only architecture/ files changed: allow
- If no code files changed: allow
- If validation report is staged: allow
- If report exists but not staged: deny (instruct to stage)
- If no report: deny (instruct to run archi-validate)
"""

import json
import os
import re
import subprocess
import sys


SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PLUGIN_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
UTILS_DIR = os.path.join(PLUGIN_ROOT, "utils")

if UTILS_DIR not in sys.path:
    sys.path.insert(0, UTILS_DIR)

import importlib.util

from hook_output import output_pre_tool_use

# Import active-change.py (hyphenated filename requires importlib)
_ac_path = os.path.join(UTILS_DIR, "active-change.py")
_ac_spec = importlib.util.spec_from_file_location("active_change", _ac_path)
_ac_module = importlib.util.module_from_spec(_ac_spec)
_ac_spec.loader.exec_module(_ac_module)
find_active_change = _ac_module.find_active_change


CODE_EXTENSIONS = {
    ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs",
    ".py", ".pyi",
    ".java", ".kt", ".kts",
    ".go",
    ".rs",
    ".c", ".cpp", ".h", ".hpp",
    ".rb",
    ".swift",
}


def run_git(args, cwd):
    """Run a git command and return stdout, or None on failure."""
    try:
        result = subprocess.run(
            ["git"] + args,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=10,
            shell=True,
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except (subprocess.TimeoutExpired, OSError, FileNotFoundError):
        pass
    return None


def is_git_commit_command(command):
    """Check if the command is a git commit."""
    if not command:
        return False
    return bool(re.search(r"\bgit\s+commit\b", command))


def get_staged_files(cwd):
    """Get list of staged file paths."""
    output = run_git(["diff", "--cached", "--name-only"], cwd)
    if not output:
        return []
    return [f for f in output.splitlines() if f]


def has_code_files(files):
    """Check if any staged file is a code file (by extension)."""
    for f in files:
        ext = os.path.splitext(f)[1].lower()
        if ext in CODE_EXTENSIONS:
            return True
    return False


def is_architecture_only(files):
    """Check if all staged files are under openspec/specs/architecture/."""
    if not files:
        return False
    return all(f.startswith("openspec/specs/architecture/") for f in files)


def find_staged_report(cwd):
    """Find an architecture-validate-*.json report in staged changes.

    Returns the report file path relative to cwd, or None if not found.
    """
    staged = get_staged_files(cwd)
    for f in staged:
        if re.match(r"openspec/changes/[\w\-]+/reports/architecture-validate-[\w\-]+\.json$", f):
            return f
    return None


def compute_staged_diff_hash(cwd):
    """Compute a hash of the current staged diff for deduplication."""
    import hashlib
    try:
        result = subprocess.run(
            ["git", "diff", "--cached"],
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=10,
            shell=True,
        )
        if result.returncode == 0 and result.stdout:
            return hashlib.sha256(result.stdout.encode()).hexdigest()[:16]
    except (subprocess.TimeoutExpired, OSError, FileNotFoundError):
        pass
    return None


def validate_staged_report(cwd):
    """Validate a staged architecture report.

    Checks that the report status is clean and the diff hash matches
    the current staged changes.

    Returns:
        (valid, report_path, error_message)
        - (True, path, "") if valid
        - (False, path, reason) if invalid
        - (False, None, reason) if no report or unreadable
    """
    report_path = find_staged_report(cwd)
    if not report_path:
        return (False, None, "no report staged")

    report_full_path = os.path.join(cwd, report_path)
    if not os.path.isfile(report_full_path):
        return (False, None, f"report file not found on disk: {report_path}")

    try:
        with open(report_full_path, "r", encoding="utf-8") as f:
            report = json.load(f)
    except (json.JSONDecodeError, OSError) as e:
        return (False, report_path, f"failed to read report: {e}")

    # Check report status
    status = report.get("status", "")
    if status == "violations_found":
        violation_count = len(report.get("violations", []))
        return (False, report_path,
                f"report status is 'violations_found' ({violation_count} violation(s)). "
                "Fix violations and re-validate before committing.")

    if status == "skipped":
        return (False, report_path,
                "report was skipped (no model to validate against). "
                "Ensure the architecture model exists before validating.")

    # Check diff hash matches
    report_hash = report.get("commit_diff_hash", "")
    current_hash = compute_staged_diff_hash(cwd)
    if report_hash and current_hash and report_hash != current_hash:
        return (False, report_path,
                f"report diff hash ({report_hash}) does not match "
                f"current staged diff hash ({current_hash}). "
                "Re-run archi-validate.py --staged to generate a fresh report.")

    return (True, report_path, "")


def model_exists(cwd):
    """Check if architecture model exists.

    Checks models/ directory for *.c4 files.
    """
    models_dir = os.path.join(cwd, "openspec", "specs", "architecture", "models")
    if os.path.isdir(models_dir):
        for f in os.listdir(models_dir):
            if f.endswith(".c4"):
                return True

    return False


def run_gate(input_data):
    """Run architecture validation gate for a git commit.

    Returns: (decision, context) where decision is "allow" or "deny"
    """
    cwd = input_data.get("cwd", "")

    # No model → nothing to validate against
    if not model_exists(cwd):
        return ("allow", "")

    staged_files = get_staged_files(cwd)

    # No staged files → allow
    if not staged_files:
        return ("allow", "")

    # Only architecture files → allow
    if is_architecture_only(staged_files):
        return ("allow", "Architecture-only commit detected. Skipping architecture validation gate.")

    # No code files → allow
    if not has_code_files(staged_files):
        return ("allow", "")

    # Check for validation report
    valid, report_path, reason = validate_staged_report(cwd)

    if valid:
        return ("allow", f"Architecture validation report '{report_path}' is clean. Gate passed.")

    if report_path:
        # Report exists but is invalid
        return ("deny", (
            f"ARCHITECTURE GATE: Commit denied. "
            f"Validation report '{report_path}' is invalid: {reason}"
        ))

    # Check if report exists but not staged (check active change's reports dir)
    changes_dir = os.path.join(cwd, "openspec", "changes")
    active = find_active_change(changes_dir, cwd)
    unstaged_report = False
    report_pattern = None
    if active:
        change_name, _ = active
        reports_dir = os.path.join(cwd, "openspec", "changes", change_name, "reports")
        if os.path.isdir(reports_dir):
            for f in os.listdir(reports_dir):
                if re.match(r"architecture-validate-[\w\-]+\.json$", f):
                    unstaged_report = True
                    report_pattern = os.path.join("openspec", "changes", change_name, "reports", f"architecture-validate-*.json")
                    break
    # Also check global fallback directory
    if not unstaged_report:
        global_reports_dir = os.path.join(cwd, "openspec", "specs", "architecture", "reports")
        if os.path.isdir(global_reports_dir):
            for f in os.listdir(global_reports_dir):
                if re.match(r"architecture-validate-[\w\-]+\.json$", f):
                    unstaged_report = True
                    report_pattern = os.path.join("openspec", "specs", "architecture", "reports", "architecture-validate-*.json")
                    break

    if unstaged_report:
        return ("deny", (
            "ARCHITECTURE GATE: Commit denied. "
            "A validation report exists but is NOT staged. "
            f"Stage the report with: git add {report_pattern}"
        ))

    # No report at all → deny with guidance
    file_list = ", ".join(staged_files[:5])
    if len(staged_files) > 5:
        file_list += f" (+{len(staged_files) - 5} more)"

    return ("deny", (
        f"ARCHITECTURE GATE: Commit denied. "
        f"No architecture validation report found for {len(staged_files)} staged file(s): {file_list}. "
        f"BEFORE committing, you MUST run architecture validation:\n"
        f"  1. Run: python plugins/dev-team/utils/archi-validate.py --project-root . --staged\n"
        f"  2. The script will generate a report at openspec/changes/<name>/reports/architecture-validate-<timestamp>.json\n"
        f"  3. Stage the report: git add openspec/changes/<name>/reports/architecture-validate-*.json\n"
        f"  4. Retry the commit\n"
        f"If there are violations, address them by updating model files in openspec/specs/architecture/models/ or fixing code before re-validating."
    ))


def main():
    input_data = json.load(sys.stdin)
    tool_name = input_data.get("tool_name", "")
    tool_input = input_data.get("tool_input", {})
    command = tool_input.get("command", "")

    # Only intercept Bash + git commit
    if tool_name != "Bash":
        output_pre_tool_use("allow", "")
        return
    if not is_git_commit_command(command) or "--amend" in command or "--no-verify" in command:
        output_pre_tool_use("allow", "")
        return

    decision, context = run_gate(input_data)
    output_pre_tool_use(decision, context)


if __name__ == "__main__":
    main()
