#!/usr/bin/env python3
"""
Hook: PreToolUse (Bash) - Check code review status before git commit.

Intercepts Bash tool calls that contain git commit commands.
If an active OpenSpec change exists:
1. Runs type/lint checks → deny if errors
2. Runs full test suite → deny if failures
3. Checks if a code review report exists in test-reports/
4. If review has BLOCK verdict with security errors → deny commit
5. If review has BLOCK verdict without security errors → allow with warning
6. If no review exists → recommend running code review first
7. Tracks multiple reviews and shows comparison with previous review
"""

import json
import os
import re
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
        count_tasks = _ac_module.count_tasks
    else:
        # Fallback: inline implementations
        def find_active_change(changes_dir, cwd=""):
            try:
                for entry in os.listdir(changes_dir):
                    entry_path = os.path.join(changes_dir, entry)
                    if not os.path.isdir(entry_path) or entry == "archive":
                        continue
                    tasks_path = os.path.join(entry_path, "tasks.md")
                    if os.path.isfile(tasks_path):
                        total, done = count_tasks(tasks_path)
                        if done < total:
                            return (entry, entry_path)
            except OSError:
                pass
            return None

        def count_tasks(tasks_path):
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
except Exception:
    def find_active_change(changes_dir, cwd=""):
        return None
    def count_tasks(tasks_path):
        return 0, 0

try:
    import importlib.util
    _lr_path = os.path.join(UTILS_DIR, "lint-runner.py")
    if os.path.isfile(_lr_path):
        _lr_spec = importlib.util.spec_from_file_location("lint_runner", _lr_path)
        _lr_module = importlib.util.module_from_spec(_lr_spec)
        _lr_spec.loader.exec_module(_lr_module)
        run_type_check = _lr_module.run_type_check
        run_lint = _lr_module.run_lint
        HAS_LINT_RUNNER = True
    else:
        HAS_LINT_RUNNER = False
except Exception:
    HAS_LINT_RUNNER = False

try:
    _tr_path = os.path.join(UTILS_DIR, "test-runner.py")
    if os.path.isfile(_tr_path):
        _tr_spec = importlib.util.spec_from_file_location("test_runner", _tr_path)
        _tr_module = importlib.util.module_from_spec(_tr_spec)
        _tr_spec.loader.exec_module(_tr_module)
        run_tests = _tr_module.run_tests
        HAS_TEST_RUNNER = True
    else:
        HAS_TEST_RUNNER = False
except Exception:
    HAS_TEST_RUNNER = False

try:
    _rc_path = os.path.join(UTILS_DIR, "report-chain.py")
    if os.path.isfile(_rc_path):
        _rc_spec = importlib.util.spec_from_file_location("report_chain", _rc_path)
        _rc_module = importlib.util.module_from_spec(_rc_spec)
        _rc_spec.loader.exec_module(_rc_module)
        check_report_chain = _rc_module.check_report_chain
        get_current_task = _rc_module.get_current_task
        HAS_REPORT_CHAIN = True
    else:
        HAS_REPORT_CHAIN = False
except Exception:
    HAS_REPORT_CHAIN = False


def run_git(args, cwd):
    """Run a git command and return stdout, or None on failure."""
    try:
        result = subprocess.run(
            ["git"] + args,
            cwd=cwd,
            capture_output=True,
            text=True,
            timeout=10,
            shell=True,  # Required on Windows to resolve PATH
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except (subprocess.TimeoutExpired, OSError, FileNotFoundError):
        pass
    return None


def get_staged_files(cwd):
    """Get list of staged files from git.

    Returns:
        List of staged file paths (relative to cwd)
    """
    output = run_git(["diff", "--cached", "--name-only"], cwd)
    if not output:
        return []
    return [f for f in output.splitlines() if f]


def is_pure_openspec_docs(files):
    """Check if all staged files are OpenSpec documentation/artifacts.

    Args:
        files: List of file paths (relative to project root)

    Returns:
        True if all files are under openspec/ and are documentation/artifacts
    """
    if not files:
        return False

    # Documentation/artifact patterns
    doc_patterns = [
        r"^openspec/changes/[^/]+/proposal\.md$",
        r"^openspec/changes/[^/]+/design\.md$",
        r"^openspec/changes/[^/]+/tasks\.md$",
        r"^openspec/changes/[^/]+/specs/.*\.md$",
        r"^openspec/changes/[^/]+/\.openspec\.yaml$",
        r"^openspec/specs/.*\.md$",
    ]

    for f in files:
        # Must be under openspec/
        if not f.startswith("openspec/"):
            return False

        # Must match one of the doc patterns
        is_doc = any(re.match(p, f) for p in doc_patterns)
        if not is_doc:
            return False

    return True


def main():
    input_data = json.load(sys.stdin)
    tool_name = input_data.get("tool_name", "")
    tool_input = input_data.get("tool_input", {})
    cwd = input_data.get("cwd", "")

    # Only intercept Bash tool calls
    if tool_name != "Bash":
        output_pre_tool_use("allow", "")
        return

    command = tool_input.get("command", "")

    # Check if this is a git commit command (skip --amend and --no-verify)
    if not is_git_commit_command(command) or "--amend" in command or "--no-verify" in command:
        output_pre_tool_use("allow", "")
        return

    changes_dir = os.path.join(cwd, "openspec", "changes")

    # No changes directory
    if not os.path.isdir(changes_dir):
        output_pre_tool_use("allow", "")
        return

    # Find active change (uses shared module with priority ordering)
    active_change = find_active_change(changes_dir, cwd)
    if not active_change:
        # No active change — fall back to basic review suggestion
        staged_context = build_staged_files_context(cwd)
        if staged_context:
            output_pre_tool_use("allow", staged_context)
        else:
            output_pre_tool_use("allow", "")
        return

    change_name, change_dir = active_change
    test_reports_dir = os.path.join(change_dir, "test-reports")

    # === Skip report chain for pure documentation commits ===
    staged_files = get_staged_files(cwd)
    if is_pure_openspec_docs(staged_files):
        # Pure openspec documentation commit — skip report chain
        context = (
            f"Documentation-only commit detected (openspec/ artifacts). "
            f"Skipping lint/test/review gates for change '{change_name}'."
        )
        output_pre_tool_use("allow", context)
        return

    # === Report Chain Check ===
    if HAS_REPORT_CHAIN:
        current_task = get_current_task(change_dir)
        if current_task:
            ok, err = check_report_chain(change_dir, current_task)
            if not ok:
                context = (
                    f"REPORT CHAIN INCOMPLETE for task {current_task}: {err}. "
                    f"Ensure you executed: test-scope → lint → scoped-test with --save-report."
                )
                output_pre_tool_use("deny", context)
                return

    # === Phase 1: Type/Lint Checks ===
    lint_context = run_lint_type_checks(cwd)
    if lint_context:
        # Lint/type errors found — deny
        output_pre_tool_use("deny", lint_context)
        return

    # === Phase 1: Full Test Suite ===
    test_context = run_full_tests(cwd)
    if test_context:
        # Test failures found — deny
        output_pre_tool_use("deny", test_context)
        return

    # Find review reports
    reviews = find_review_reports(test_reports_dir)

    if not reviews:
        # No review done yet — recommend review
        staged_context = build_staged_files_context(cwd)
        context = (
            f"No code review found for change '{change_name}'. "
            f"RECOMMENDED: Run code review before committing. "
            f"Use the Agent tool with subagent_type 'code-review' to review staged changes. "
            f"Review results will be saved to openspec/changes/{change_name}/test-reports/."
        )
        if staged_context:
            context += " " + staged_context
        output_pre_tool_use("allow", context)
        return

    # Get latest review
    latest_review = reviews[-1]
    verdict = parse_review_verdict(latest_review["content"])
    error_count = count_review_errors(latest_review["content"])
    security_errors = find_security_errors(latest_review["content"])

    # Phase 2.5: Security errors → deny
    if security_errors:
        error_list = "; ".join(security_errors[:3])
        if len(security_errors) > 3:
            error_list += f" (+{len(security_errors) - 3} more)"
        context = (
            f"CRITICAL: {len(security_errors)} security issue(s) detected in code review for '{change_name}'. "
            f"Commit BLOCKED. Issues: {error_list}. "
            f"Fix the security issues before committing. "
            f"Review report: test-reports/{os.path.basename(latest_review['path'])}"
        )
        output_pre_tool_use("deny", context)
        return

    # Phase 2.6: Non-security errors → ERROR→Task instruction
    if verdict == "BLOCK" or error_count > 0:
        comparison = ""
        if len(reviews) > 1:
            prev_review = reviews[-2]
            prev_verdict = parse_review_verdict(prev_review["content"])
            prev_errors = count_review_errors(prev_review["content"])
            diff = prev_errors - error_count
            trend = f"({diff} fixed)" if diff > 0 else f"({abs(diff)} new)" if diff < 0 else "(same)"
            comparison = (
                f" Review history: {prev_verdict} ({prev_errors} errors) → {verdict} ({error_count} errors) {trend}."
            )

        review_path = latest_review["path"]
        tasks_md_path = os.path.join(change_dir, "tasks.md")

        context = (
            f"REVIEW BLOCK: {error_count} error(s) found in '{change_name}'."
            f"{comparison}\n"
            f"BEFORE committing, you MUST:\n"
            f"1. Generate fix tasks:\n"
            f"   python plugin/utils/review-parser.py \"{review_path}\" "
            f"     --generate-tasks --append-to \"{tasks_md_path}\"\n"
            f"2. Record loop state:\n"
            f"   python plugin/utils/review-loop-state.py \"{change_dir}\" "
            f"     --record {error_count} {error_count}\n"
            f"3. Re-enter apply: invoke the openspec-apply-change skill "
            f"   to implement the fix tasks.\n"
            f"This ensures all review errors are addressed before commit."
        )
        output_pre_tool_use("allow", context)
        return

    # Review passed — allow with confirmation
    context = (
        f"Code review PASSED for change '{change_name}'. "
        f"Proceeding with commit."
    )
    output_pre_tool_use("allow", context)


def is_git_commit_command(command):
    """Check if the command is a git commit."""
    patterns = [
        r"\bgit\s+commit\b",
        r"\bgit-commit\b",
    ]
    for pattern in patterns:
        if re.search(pattern, command):
            return True
    return False


def find_review_reports(test_reports_dir):
    """Find all code review reports in test-reports/, sorted by timestamp.

    Returns list of dicts: [{"path": ..., "content": ...}]
    """
    reviews = []
    if not os.path.isdir(test_reports_dir):
        return reviews

    try:
        for f in os.listdir(test_reports_dir):
            if re.match(r"code-review-\d{8}-\d{6}\.md$", f):
                filepath = os.path.join(test_reports_dir, f)
                try:
                    with open(filepath, "r", encoding="utf-8") as fh:
                        content = fh.read()
                    reviews.append({
                        "path": filepath,
                        "content": content,
                    })
                except OSError:
                    pass
    except OSError:
        pass

    # Sort by filename (which contains timestamp)
    reviews.sort(key=lambda r: os.path.basename(r["path"]))
    return reviews


def parse_review_verdict(content):
    """Parse the verdict from a code review report.

    Returns: "PASS", "BLOCK", or "UNKNOWN"
    """
    # Check for BLOCK first (more restrictive)
    if re.search(r"\*\*BLOCK\*\*|Verdict:\s*BLOCK|##\s*Verdict\s*\n\s*BLOCK", content, re.IGNORECASE):
        return "BLOCK"
    if re.search(r"\*\*PASS\*\*|Verdict:\s*PASS|##\s*Verdict\s*\n\s*PASS", content, re.IGNORECASE):
        return "PASS"
    return "UNKNOWN"


def count_review_errors(content):
    """Count ERROR entries in a code review report."""
    errors = re.findall(r"\[ERROR-\d+\]", content)
    return len(errors)


def find_security_errors(content):
    """Find security-related errors in a code review report.

    Returns list of security error descriptions.
    """
    security_errors = []

    # Find ERROR blocks that mention security categories
    # Match pattern: [ERROR-N] file:line followed by Category: Security
    error_sections = re.split(r"(?=####\s*\[ERROR-)", content)

    for section in error_sections:
        if not re.match(r"####\s*\[ERROR-", section):
            continue

        if re.search(r"\*\*Category\*\*:\s*Security", section, re.IGNORECASE):
            # Extract brief description
            desc_match = re.search(
                r"\*\*Description\*\*:\s*(.+?)(?:\n|$)",
                section,
            )
            loc_match = re.search(r"\[ERROR-(\d+)\]\s*(.+?)(?:\n|$)", section)

            if desc_match:
                security_errors.append(desc_match.group(1).strip())
            elif loc_match:
                security_errors.append(loc_match.group(2).strip()[:80])

    return security_errors


def build_staged_files_context(cwd):
    """Build context about staged files for review recommendation."""
    staged_stat = run_git(["diff", "--cached", "--stat"], cwd)
    if not staged_stat:
        return ""

    staged_files = parse_staged_files(staged_stat)
    if not staged_files:
        return ""

    file_list = ", ".join(staged_files[:5])
    if len(staged_files) > 5:
        file_list += f"... (+{len(staged_files) - 5} more)"

    return f"Staged files ({len(staged_files)}): {file_list}"


def parse_staged_files(stat_output):
    """Parse staged files from git diff --cached --stat output."""
    files = []
    for line in stat_output.splitlines():
        if "insertion" in line or "deletion" in line or "|" not in line:
            continue
        parts = line.split("|")
        if parts:
            filename = parts[0].strip()
            if filename:
                files.append(filename)
    return files


def run_lint_type_checks(cwd):
    """Run type and lint checks. Returns context string if errors found, None if passed."""
    if not HAS_LINT_RUNNER:
        return None

    try:
        type_results = run_type_check(cwd)
        lint_results = run_lint(cwd)

        failed_checks = []

        for result in type_results:
            if not result.success:
                failed_checks.append(
                    f"[{result.linter}] {result.errors} type error(s)"
                )

        for result in lint_results:
            if not result.success:
                failed_checks.append(
                    f"[{result.linter}] {result.errors} lint error(s), {result.warnings} warning(s)"
                )

        if failed_checks:
            details = "; ".join(failed_checks)
            return (
                f"LINT/TYPE GATE: Commit blocked — {details}. "
                f"Fix type and lint errors before committing."
            )

    except Exception:
        # Non-blocking: lint failures shouldn't crash the hook
        pass

    return None


def run_full_tests(cwd):
    """Run full test suite. Returns context string if failures found, None if passed."""
    if not HAS_TEST_RUNNER:
        return None

    try:
        result = run_tests(cwd)

        # If no test framework detected, don't block commit
        if result.framework == "unknown":
            return None

        if not result.success:
            return (
                f"TEST GATE: Commit blocked — {result.failed}/{result.total} test(s) failed. "
                f"Fix failing tests before committing. "
                f"Framework: {result.framework}"
            )

    except Exception:
        # Non-blocking: test runner failures shouldn't crash the hook
        pass

    return None


if __name__ == "__main__":
    main()
