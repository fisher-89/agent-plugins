#!/usr/bin/env python3
"""
Hook: PreToolUse (Bash) - Architecture validation gate for git commit.

Intercepts Bash tool calls that contain git commit commands.
Checks that staged code changes have a corresponding architecture validation
report (validate-*.json) in openspec/architecture/reports/.

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

from hook_output import output_pre_tool_use


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
    """Check if all staged files are under openspec/architecture/."""
    if not files:
        return False
    return all(f.startswith("openspec/architecture/") for f in files)


def has_validation_report_staged(cwd):
    """Check if a validate-*.json report is in staged changes."""
    staged = get_staged_files(cwd)
    for f in staged:
        if re.match(r"openspec/architecture/reports/validate-[\w\-]+\.json$", f):
            return True
    return False


def model_exists(cwd):
    """Check if architecture model exists.

    Checks models/ directory for *.c4 files first, then legacy model.c4.
    """
    models_dir = os.path.join(cwd, "openspec", "architecture", "models")
    if os.path.isdir(models_dir):
        for f in os.listdir(models_dir):
            if f.endswith(".c4"):
                return True

    # Backward compat: also check legacy model.c4
    model_path = os.path.join(cwd, "openspec", "architecture", "model.c4")
    return os.path.isfile(model_path)


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
    if has_validation_report_staged(cwd):
        return ("allow", "Architecture validation report found in staged changes. Gate passed.")

    # Check if report exists but not staged
    reports_dir = os.path.join(cwd, "openspec", "architecture", "reports")
    unstaged_report = False
    if os.path.isdir(reports_dir):
        for f in os.listdir(reports_dir):
            if re.match(r"validate-[\w\-]+\.json$", f):
                unstaged_report = True
                break

    if unstaged_report:
        return ("deny", (
            "ARCHITECTURE GATE: Commit denied. "
            "A validation report exists in openspec/architecture/reports/ but is NOT staged. "
            "Stage the report with: git add openspec/architecture/reports/validate-*.json"
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
        f"  2. The script will generate a report at openspec/architecture/reports/validate-<timestamp>.json\n"
        f"  3. Stage the report: git add openspec/architecture/reports/validate-*.json\n"
        f"  4. Retry the commit\n"
        f"If there are violations, address them by updating model files in openspec/architecture/models/ or fixing code before re-validating."
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
