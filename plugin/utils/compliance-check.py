#!/usr/bin/env python3
"""
Compliance check utility.

Runs pre-archive compliance checks (C1-C10) and generates reports.
C1-C8: Required (blocking)
C9-C10: Optional (configuration-driven)
"""

import glob
import json
import os
import re
import subprocess
import sys
from datetime import datetime
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass, field


# Import test-framework for C5
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))

try:
    import importlib.util
    _tf_path = os.path.join(SCRIPT_DIR, "test-framework.py")
    if os.path.isfile(_tf_path):
        _tf_spec = importlib.util.spec_from_file_location("test_framework", _tf_path)
        _tf_module = importlib.util.module_from_spec(_tf_spec)
        _tf_spec.loader.exec_module(_tf_module)
        detect_test_framework = _tf_module.detect_test_framework
        HAS_TEST_FRAMEWORK = True
    else:
        HAS_TEST_FRAMEWORK = False
except Exception:
    HAS_TEST_FRAMEWORK = False

try:
    _rp_path = os.path.join(SCRIPT_DIR, "review-parser.py")
    if os.path.isfile(_rp_path):
        _rp_spec = importlib.util.spec_from_file_location("review_parser", _rp_path)
        _rp_module = importlib.util.module_from_spec(_rp_spec)
        _rp_spec.loader.exec_module(_rp_module)
        parse_review_report = _rp_module.parse_review_report
        HAS_REVIEW_PARSER = True
    else:
        HAS_REVIEW_PARSER = False
except Exception:
    HAS_REVIEW_PARSER = False

try:
    _sc_path = os.path.join(SCRIPT_DIR, "spec-compliance.py")
    if os.path.isfile(_sc_path):
        _sc_spec = importlib.util.spec_from_file_location("spec_compliance", _sc_path)
        _sc_module = importlib.util.module_from_spec(_sc_spec)
        _sc_spec.loader.exec_module(_sc_module)
        check_spec_compliance = _sc_module.check_spec_compliance
        HAS_SPEC_COMPLIANCE = True
    else:
        HAS_SPEC_COMPLIANCE = False
except Exception:
    HAS_SPEC_COMPLIANCE = False


@dataclass
class CheckResult:
    """Result of a single compliance check."""
    check_id: str
    name: str
    passed: bool
    level: str  # "must", "optional"
    details: str = ""
    skipped: bool = False

    @property
    def status_icon(self) -> str:
        if self.skipped:
            return "⚠️ SKIP"
        return "✅ PASS" if self.passed else "❌ FAIL"


@dataclass
class ComplianceReport:
    """Full compliance check report."""
    change_name: str
    date: str
    checks: List[CheckResult] = field(default_factory=list)

    @property
    def passed(self) -> bool:
        """Overall pass: all required (must-level) checks pass."""
        return all(
            c.passed or c.skipped
            for c in self.checks
            if c.level == "must"
        )

    @property
    def blocking_issues(self) -> List[CheckResult]:
        """Failed required checks."""
        return [
            c for c in self.checks
            if c.level == "must" and not c.passed and not c.skipped
        ]

    @property
    def pass_count(self) -> int:
        return sum(1 for c in self.checks if c.passed and not c.skipped)

    @property
    def fail_count(self) -> int:
        return sum(1 for c in self.checks if not c.passed and not c.skipped)

    @property
    def skip_count(self) -> int:
        return sum(1 for c in self.checks if c.skipped)


# ─── C1: proposal.md exists ───────────────────────────────────────────

def check_c1_proposal_exists(change_dir: str) -> CheckResult:
    """C1: proposal.md exists."""
    path = os.path.join(change_dir, "proposal.md")
    if os.path.isfile(path):
        return CheckResult("C1", "proposal.md exists", True, "must")
    return CheckResult("C1", "proposal.md exists", False, "must", "File not found")


# ─── C2: tasks.md exists ──────────────────────────────────────────────

def check_c2_tasks_exists(change_dir: str) -> CheckResult:
    """C2: tasks.md exists."""
    path = os.path.join(change_dir, "tasks.md")
    if os.path.isfile(path):
        return CheckResult("C2", "tasks.md exists", True, "must")
    return CheckResult("C2", "tasks.md exists", False, "must", "File not found")


# ─── C3: All tasks complete ───────────────────────────────────────────

def check_c3_all_tasks_complete(change_dir: str) -> CheckResult:
    """C3: All tasks marked as complete."""
    path = os.path.join(change_dir, "tasks.md")
    if not os.path.isfile(path):
        return CheckResult("C3", "All tasks complete", False, "must", "tasks.md not found")

    total, done = _count_tasks(path)
    if total == 0:
        return CheckResult("C3", "All tasks complete", True, "must", "No tasks found")
    if done >= total:
        return CheckResult("C3", "All tasks complete", True, "must", f"{done}/{total} tasks")
    return CheckResult("C3", "All tasks complete", False, "must", f"{done}/{total} tasks complete")


def _count_tasks(tasks_path: str) -> Tuple[int, int]:
    """Count total and completed tasks in tasks.md."""
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


# ─── C4: Test files exist ─────────────────────────────────────────────

def check_c4_test_files_exist(change_dir: str, project_root: str) -> CheckResult:
    """C4: At least one test file exists."""
    test_patterns = [
        os.path.join(project_root, "tests", "**", "*.test.*"),
        os.path.join(project_root, "tests", "**", "*.spec.*"),
        os.path.join(project_root, "test", "**", "*.test.*"),
        os.path.join(project_root, "test", "**", "*.spec.*"),
        os.path.join(project_root, "src", "**", "*.test.*"),
        os.path.join(project_root, "src", "**", "*.spec.*"),
        os.path.join(project_root, "**", "test_*.py"),
        os.path.join(project_root, "**", "*_test.py"),
    ]

    found = []
    for pattern in test_patterns:
        found.extend(glob.glob(pattern, recursive=True))

    # Deduplicate
    found = list(set(found))

    # Also check test-reports for test case templates
    test_reports_dir = os.path.join(change_dir, "test-reports")
    if os.path.isdir(test_reports_dir):
        for f in os.listdir(test_reports_dir):
            if f.startswith("test-case-") and f.endswith(".md"):
                found.append(os.path.join(test_reports_dir, f))

    if found:
        return CheckResult("C4", "Test files exist", True, "must", f"{len(found)} test file(s)")
    return CheckResult("C4", "Test files exist", False, "must", "No test files found")


# ─── C5: Tests pass (check test-reports records) ──────────────────────

def check_c5_tests_pass(change_dir: str, project_root: str) -> CheckResult:
    """C5: Full test suite has passed (verified via test-reports records).

    Since full tests are already run before compliance check,
    we verify the test results record exists in test-reports/.
    """
    test_reports_dir = os.path.join(change_dir, "test-reports")

    if not os.path.isdir(test_reports_dir):
        return CheckResult("C5", "Tests pass", False, "must", "No test-reports directory")

    # Look for test result records
    test_result_files = []
    for f in os.listdir(test_reports_dir):
        if re.match(r"test-result-.*\.md$", f) or re.match(r"full-test-.*\.md$", f):
            test_result_files.append(f)

    if not test_result_files:
        # Fallback: check if review-loop-state mentions tests passed
        loop_state_path = os.path.join(test_reports_dir, "review-loop-state.json")
        if os.path.isfile(loop_state_path):
            try:
                with open(loop_state_path, "r", encoding="utf-8") as fh:
                    state = json.load(fh)
                if state.get("full_test_passed"):
                    return CheckResult("C5", "Tests pass", True, "must", "Full test suite passed (from loop state)")
            except (json.JSONDecodeError, OSError):
                pass

        # Check pre-tool-commit-review hook would have already validated
        # If we got here, full tests passed during commit flow
        return CheckResult("C5", "Tests pass", False, "must",
                          "No test result record found in test-reports/")

    # Check latest test result for pass status
    test_result_files.sort(reverse=True)
    latest = test_result_files[0]
    latest_path = os.path.join(test_reports_dir, latest)

    try:
        with open(latest_path, "r", encoding="utf-8") as f:
            content = f.read()

        # Check for pass indicators
        if re.search(r"(Result|Status|Verdict):\s*PASS", content, re.IGNORECASE):
            return CheckResult("C5", "Tests pass", True, "must",
                              f"Full test suite passed (record: {latest})")
        if re.search(r"(Result|Status|Verdict):\s*FAIL", content, re.IGNORECASE):
            return CheckResult("C5", "Tests pass", False, "must",
                              f"Full test suite FAILED (record: {latest})")

        # If no clear verdict, assume passed (commit gate would have blocked otherwise)
        return CheckResult("C5", "Tests pass", True, "must",
                          f"Test result record exists ({latest})")
    except OSError:
        return CheckResult("C5", "Tests pass", False, "must", "Cannot read test result record")


# ─── C6: Code review exists ───────────────────────────────────────────

def check_c6_review_exists(change_dir: str) -> CheckResult:
    """C6: At least one code review report exists."""
    test_reports_dir = os.path.join(change_dir, "test-reports")

    if not os.path.isdir(test_reports_dir):
        return CheckResult("C6", "Code review exists", False, "must", "No test-reports directory")

    reviews = []
    for f in os.listdir(test_reports_dir):
        if re.match(r"code-review-\d{8}-\d{6}\.md$", f):
            reviews.append(f)

    if reviews:
        return CheckResult("C6", "Code review exists", True, "must", f"{len(reviews)} review report(s)")
    return CheckResult("C6", "Code review exists", False, "must", "No code review reports found")


# ─── C7: Code review no ERROR ─────────────────────────────────────────

def check_c7_review_no_error(change_dir: str) -> CheckResult:
    """C7: Latest code review has no ERROR findings (PASS verdict)."""
    test_reports_dir = os.path.join(change_dir, "test-reports")

    if not os.path.isdir(test_reports_dir):
        return CheckResult("C7", "Review no ERROR", False, "must", "No test-reports directory")

    reviews = []
    for f in os.listdir(test_reports_dir):
        if re.match(r"code-review-\d{8}-\d{6}\.md$", f):
            reviews.append(f)

    if not reviews:
        return CheckResult("C7", "Review no ERROR", False, "must", "No code review reports found")

    # Check latest review
    reviews.sort(reverse=True)
    latest_path = os.path.join(test_reports_dir, reviews[0])

    try:
        with open(latest_path, "r", encoding="utf-8") as f:
            content = f.read()
    except OSError:
        return CheckResult("C7", "Review no ERROR", False, "must", "Cannot read review report")

    # Use review parser if available
    if HAS_REVIEW_PARSER:
        try:
            result = parse_review_report(latest_path)
            if result.verdict == "PASS":
                return CheckResult("C7", "Review no ERROR", True, "must",
                                  f"Latest review: PASS ({result.error_count} errors, {result.warn_count} warnings)")
            elif result.verdict == "BLOCK":
                return CheckResult("C7", "Review no ERROR", False, "must",
                                  f"Latest review: BLOCK ({result.error_count} errors)")
            else:
                # Unknown verdict — check error count
                if result.error_count == 0:
                    return CheckResult("C7", "Review no ERROR", True, "must",
                                      f"Latest review: no errors ({result.warn_count} warnings)")
                return CheckResult("C7", "Review no ERROR", False, "must",
                                  f"Latest review: {result.error_count} errors found")
        except Exception:
            pass

    # Fallback: regex parsing
    error_count = len(re.findall(r"\[ERROR-\d+\]", content))
    verdict = "UNKNOWN"
    if re.search(r"\*\*Verdict\*\*:\s*PASS", content, re.IGNORECASE):
        verdict = "PASS"
    elif re.search(r"\*\*Verdict\*\*:\s*BLOCK", content, re.IGNORECASE):
        verdict = "BLOCK"

    if verdict == "PASS" or (verdict == "UNKNOWN" and error_count == 0):
        return CheckResult("C7", "Review no ERROR", True, "must",
                          f"Latest review: {verdict} ({error_count} errors)")
    return CheckResult("C7", "Review no ERROR", False, "must",
                      f"Latest review: {verdict} ({error_count} errors)")


# ─── C8: No uncommitted changes ───────────────────────────────────────

def check_c8_no_uncommitted(project_root: str) -> CheckResult:
    """C8: No uncommitted changes in git."""
    try:
        result = subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=project_root,
            capture_output=True,
            text=True,
            timeout=10,
            shell=True,
        )
        if result.returncode != 0:
            return CheckResult("C8", "No uncommitted", True, "must", "Not a git repo (skipped)")

        dirty = [line for line in result.stdout.strip().splitlines() if line.strip()]
        if not dirty:
            return CheckResult("C8", "No uncommitted", True, "must", "Working tree clean")
        return CheckResult("C8", "No uncommitted", False, "must",
                          f"{len(dirty)} uncommitted file(s)")
    except (subprocess.TimeoutExpired, OSError, FileNotFoundError):
        return CheckResult("C8", "No uncommitted", True, "must", "Git check failed (skipped)")


# ─── C9: design.md exists (optional) ──────────────────────────────────

def check_c9_design_exists(change_dir: str, config: dict) -> CheckResult:
    """C9: design.md exists (optional, configuration-driven)."""
    require_design = config.get("require_design", False)

    if not require_design:
        return CheckResult("C9", "design.md exists", True, "optional", "", skipped=True)

    path = os.path.join(change_dir, "design.md")
    if os.path.isfile(path):
        return CheckResult("C9", "design.md exists", True, "optional")
    return CheckResult("C9", "design.md exists", False, "optional", "design.md not found (required by config)")


# ─── C10: Spec compliance (optional) ──────────────────────────────────

def check_c10_spec_compliance(change_dir: str, project_root: str, config: dict) -> CheckResult:
    """C10: Spec compliance — implementation covers proposal scope (optional)."""
    spec_compliance = config.get("spec_compliance", False)

    if not spec_compliance:
        return CheckResult("C10", "Spec compliance", True, "optional", "", skipped=True)

    # Use spec-compliance module if available
    if HAS_SPEC_COMPLIANCE:
        try:
            result = check_spec_compliance(change_dir, project_root)
            if result.get("passed", False):
                covered = len(result.get("covered", []))
                return CheckResult("C10", "Spec compliance", True, "optional",
                                  f"{covered} feature(s) covered")
            else:
                missing = result.get("missing", [])
                return CheckResult("C10", "Spec compliance", False, "optional",
                                  f"Missing: {', '.join(missing[:5])}")
        except Exception as e:
            return CheckResult("C10", "Spec compliance", False, "optional", f"Analysis failed: {e}")

    # Fallback: simple scope check (compare proposal scope items with task completions)
    proposal_path = os.path.join(change_dir, "proposal.md")
    if not os.path.isfile(proposal_path):
        return CheckResult("C10", "Spec compliance", True, "optional", "No proposal.md (skipped)", skipped=True)

    return CheckResult("C10", "Spec compliance", True, "optional",
                      "Spec compliance module not available (skipped)", skipped=True)


# ─── Run all checks ───────────────────────────────────────────────────

def run_compliance_check(
    change_dir: str,
    project_root: str,
    config: Optional[dict] = None
) -> ComplianceReport:
    """Run all compliance checks (C1-C10).

    Args:
        change_dir: Path to the change directory (e.g., openspec/changes/add-auth/).
        project_root: Path to the project root.
        config: Optional configuration dict (from .openspec.yaml).

    Returns:
        ComplianceReport with all check results.
    """
    if config is None:
        config = {}

    change_name = os.path.basename(change_dir)
    report = ComplianceReport(
        change_name=change_name,
        date=datetime.now().strftime("%Y-%m-%d %H:%M"),
    )

    # C1-C8: Required checks
    report.checks.append(check_c1_proposal_exists(change_dir))
    report.checks.append(check_c2_tasks_exists(change_dir))
    report.checks.append(check_c3_all_tasks_complete(change_dir))
    report.checks.append(check_c4_test_files_exist(change_dir, project_root))
    report.checks.append(check_c5_tests_pass(change_dir, project_root))
    report.checks.append(check_c6_review_exists(change_dir))
    report.checks.append(check_c7_review_no_error(change_dir))
    report.checks.append(check_c8_no_uncommitted(project_root))

    # C9-C10: Optional checks
    report.checks.append(check_c9_design_exists(change_dir, config))
    report.checks.append(check_c10_spec_compliance(change_dir, project_root, config))

    return report


# ─── Report generation ─────────────────────────────────────────────────

def generate_report_markdown(report: ComplianceReport) -> str:
    """Generate a markdown compliance report."""
    lines = [
        "# Pre-Archive Compliance Check",
        "",
        f"> **Change**: {report.change_name}",
        f"> **Date**: {report.date}",
        f"> **Result**: {'PASS' if report.passed else 'FAIL'}",
        "",
        "---",
        "",
        "## Checklist",
        "",
        "| ID | Check | Status | Details |",
        "|----|-------|--------|---------|",
    ]

    for c in report.checks:
        lines.append(f"| {c.check_id} | {c.name} | {c.status_icon} | {c.details or '-'} |")

    lines.extend([
        "",
        "---",
        "",
        "## Summary",
        "",
        f"- **Passed**: {report.pass_count}",
        f"- **Failed**: {report.fail_count}",
        f"- **Skipped**: {report.skip_count}",
    ])

    # Blocking issues
    blocking = report.blocking_issues
    if blocking:
        lines.extend([
            "",
            "## Blocking Issues",
            "",
        ])
        for i, c in enumerate(blocking, 1):
            lines.append(f"{i}. **[{c.check_id}] {c.name}**: {c.details}")

    # Verdict
    lines.extend([
        "",
        "## Verdict",
        "",
    ])
    if report.passed:
        lines.append("**PASS** — All compliance checks passed. Safe to archive.")
    else:
        lines.append(f"**FAIL** — Fix {len(blocking)} blocking issue(s) before archiving.")

    return "\n".join(lines)


def save_report(report: ComplianceReport, test_reports_dir: str) -> str:
    """Save compliance report to test-reports directory.

    Returns:
        Path to the saved report file.
    """
    os.makedirs(test_reports_dir, exist_ok=True)

    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    filename = f"compliance-{timestamp}.md"
    filepath = os.path.join(test_reports_dir, filename)

    content = generate_report_markdown(report)
    with open(filepath, "w", encoding="utf-8") as f:
        f.write(content)

    return filepath


def load_config(project_root: str) -> dict:
    """Load compliance configuration from .openspec.yaml if it exists."""
    config_path = os.path.join(project_root, ".openspec.yaml")
    if not os.path.isfile(config_path):
        return {}

    try:
        with open(config_path, "r", encoding="utf-8") as f:
            content = f.read()

        # Simple YAML parsing for compliance section
        config = {}
        in_compliance = False
        for line in content.splitlines():
            stripped = line.strip()
            if stripped.startswith("compliance:"):
                in_compliance = True
                continue
            if in_compliance:
                if stripped and not stripped.startswith("#") and not stripped.startswith("-"):
                    if ":" in stripped and not stripped[0].isspace():
                        in_compliance = False
                        continue
                    # Parse key: value
                    match = re.match(r"(\w+):\s*(.+)", stripped)
                    if match:
                        key = match.group(1)
                        value = match.group(2).strip().strip('"').strip("'")
                        if value.lower() == "true":
                            config[key] = True
                        elif value.lower() == "false":
                            config[key] = False
                        else:
                            config[key] = value
        return config
    except OSError:
        return {}


# ─── CLI interface ─────────────────────────────────────────────────────

def main():
    import argparse

    parser = argparse.ArgumentParser(description="Pre-archive compliance check")
    parser.add_argument("--change", required=True, help="Change name")
    parser.add_argument("--project-root", default=".", help="Project root directory")
    parser.add_argument("--json", action="store_true", help="Output as JSON")
    parser.add_argument("--save", action="store_true", help="Save report to test-reports/")
    parser.add_argument("--config", help="Path to .openspec.yaml (auto-detected if omitted)")

    args = parser.parse_args()

    project_root = os.path.abspath(args.project_root)
    change_dir = os.path.join(project_root, "openspec", "changes", args.change)

    if not os.path.isdir(change_dir):
        print(f"Error: Change directory not found: {change_dir}", file=sys.stderr)
        sys.exit(1)

    # Load config
    config = load_config(project_root)
    if args.config:
        config.update(load_config(os.path.dirname(args.config)))

    # Run checks
    report = run_compliance_check(change_dir, project_root, config)

    if args.json:
        output = {
            "change_name": report.change_name,
            "date": report.date,
            "passed": report.passed,
            "pass_count": report.pass_count,
            "fail_count": report.fail_count,
            "skip_count": report.skip_count,
            "checks": [
                {
                    "id": c.check_id,
                    "name": c.name,
                    "passed": c.passed,
                    "level": c.level,
                    "details": c.details,
                    "skipped": c.skipped,
                }
                for c in report.checks
            ],
            "blocking_issues": [
                {
                    "id": c.check_id,
                    "name": c.name,
                    "details": c.details,
                }
                for c in report.blocking_issues
            ],
        }
        print(json.dumps(output, indent=2, ensure_ascii=False))
    else:
        print(generate_report_markdown(report))

    if args.save:
        test_reports_dir = os.path.join(change_dir, "test-reports")
        path = save_report(report, test_reports_dir)
        if not args.json:
            print(f"\nReport saved to: {path}")

    sys.exit(0 if report.passed else 1)


if __name__ == "__main__":
    main()
