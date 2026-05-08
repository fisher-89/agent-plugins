#!/usr/bin/env python3
"""
Deviation check utility.

Detects and alerts on deviations between proposal scope and implementation.
Used during apply-change and archive-change to catch:
- Missing features (scope drift)
- Extra features (scope creep)
- Implementation mismatches

Integrates with spec-compliance.py for analysis.
"""

import os
import re
import sys
import json
from datetime import datetime
from typing import Dict, List, Optional
from dataclasses import dataclass, field

# Import spec-compliance for analysis
try:
    import importlib.util
    _sc_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "spec-compliance.py")
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
class Deviation:
    """A single deviation item."""
    type: str  # "missing", "extra", "mismatch"
    severity: str  # "error", "warning", "info"
    feature: str
    details: str
    evidence: str = ""


@dataclass
class DeviationReport:
    """Full deviation check report."""
    change_name: str
    date: str
    deviations: List[Deviation] = field(default_factory=list)
    passed: bool = True

    @property
    def errors(self) -> List[Deviation]:
        return [d for d in self.deviations if d.severity == "error"]

    @property
    def warnings(self) -> List[Deviation]:
        return [d for d in self.deviations if d.severity == "warning"]

    @property
    def info(self) -> List[Deviation]:
        return [d for d in self.deviations if d.severity == "info"]


# ─── Deviation detection ──────────────────────────────────────────────

def check_deviations(
    change_dir: str,
    project_root: str,
    config: Optional[dict] = None
) -> DeviationReport:
    """Run deviation detection between proposal scope and implementation.

    Args:
        change_dir: Path to the change directory.
        project_root: Path to the project root.
        config: Optional configuration dict.

    Returns:
        DeviationReport with all detected deviations.
    """
    if config is None:
        config = {}

    change_name = os.path.basename(change_dir)
    report = DeviationReport(
        change_name=change_name,
        date=datetime.now().strftime("%Y-%m-%d %H:%M"),
    )

    if not HAS_SPEC_COMPLIANCE:
        report.deviations.append(Deviation(
            type="mismatch",
            severity="warning",
            feature="spec-compliance module",
            details="Spec compliance module not available — deviation check skipped",
        ))
        return report

    # Run spec compliance analysis
    try:
        result = check_spec_compliance(change_dir, project_root)
    except Exception as e:
        report.deviations.append(Deviation(
            type="mismatch",
            severity="error",
            feature="spec compliance analysis",
            details=f"Analysis failed: {e}",
        ))
        report.passed = False
        return report

    # Process missing features
    for i, feature in enumerate(result.get("missing", [])):
        details_list = result.get("missing_details", [])
        details = details_list[i] if i < len(details_list) else {}
        evidence = details.get("reason", "Not found in implementation")

        severity = config.get("missing_severity", "error")
        report.deviations.append(Deviation(
            type="missing",
            severity=severity,
            feature=feature,
            details=f"Feature mentioned in proposal but not implemented",
            evidence=evidence,
        ))
        if severity == "error":
            report.passed = False

    # Process extra features
    for i, feature in enumerate(result.get("extra", [])):
        details_list = result.get("extra_details", [])
        details = details_list[i] if i < len(details_list) else {}

        severity = config.get("extra_severity", "warning")
        report.deviations.append(Deviation(
            type="extra",
            severity=severity,
            feature=feature,
            details="Implemented but not mentioned in proposal",
            evidence=details.get("file", ""),
        ))
        # Extra features are warnings by default, not errors

    return report


# ─── Alert formatting ──────────────────────────────────────────────────

def format_alert(report: DeviationReport) -> str:
    """Format deviation report as an alert message.

    Returns a concise, actionable alert for display during apply/archive.
    """
    if not report.deviations:
        return ""

    lines = []
    lines.append("## Scope Deviation Alert")
    lines.append("")
    lines.append(f"Change: **{report.change_name}**")
    lines.append("")

    # Group by severity
    errors = report.errors
    warnings = report.warnings
    info = report.info

    if errors:
        lines.append(f"### Errors ({len(errors)})")
        lines.append("")
        for d in errors:
            lines.append(f"- **{d.feature}**: {d.details}")
            if d.evidence:
                lines.append(f"  - {d.evidence}")
        lines.append("")

    if warnings:
        lines.append(f"### Warnings ({len(warnings)})")
        lines.append("")
        for d in warnings:
            lines.append(f"- **{d.feature}**: {d.details}")
            if d.evidence:
                lines.append(f"  - {d.evidence}")
        lines.append("")

    if info:
        lines.append(f"### Info ({len(info)})")
        lines.append("")
        for d in info:
            lines.append(f"- **{d.feature}**: {d.details}")
        lines.append("")

    # Summary
    if errors:
        lines.append("---")
        lines.append("")
        lines.append(f"**ACTION REQUIRED**: Fix {len(errors)} error(s) before archiving.")
        lines.append("- Update proposal to remove unimplemented features, OR")
        lines.append("- Implement the missing features")
    elif warnings:
        lines.append("---")
        lines.append("")
        lines.append(f"**NOTE**: {len(warnings)} warning(s) detected. Review and decide if acceptable.")

    return "\n".join(lines)


def format_inline_alert(report: DeviationReport) -> str:
    """Format a compact inline alert for console display.

    Single-line format suitable for hook output.
    """
    if not report.deviations:
        return ""

    errors = len(report.errors)
    warnings = len(report.warnings)

    if errors and warnings:
        return f"⚠️ Deviation detected: {errors} error(s), {warnings} warning(s)"
    elif errors:
        return f"❌ Deviation detected: {errors} error(s)"
    elif warnings:
        return f"⚠️ Deviation detected: {warnings} warning(s)"
    else:
        return f"ℹ️ Deviation detected: {len(report.info)} info item(s)"


# ─── Report persistence ────────────────────────────────────────────────

def generate_report_markdown(report: DeviationReport) -> str:
    """Generate a full markdown deviation report."""
    lines = [
        "# Deviation Check Report",
        "",
        f"> **Change**: {report.change_name}",
        f"> **Date**: {report.date}",
        f"> **Status**: {'PASS' if report.passed else 'FAIL'}",
        "",
        "---",
        "",
    ]

    if not report.deviations:
        lines.append("No deviations detected. Implementation matches proposal scope.")
        return "\n".join(lines)

    lines.append("## Deviations")
    lines.append("")
    lines.append("| Type | Severity | Feature | Details | Evidence |")
    lines.append("|------|----------|---------|---------|----------|")

    for d in report.deviations:
        lines.append(f"| {d.type} | {d.severity} | {d.feature} | {d.details} | {d.evidence or '-'} |")

    lines.extend(["", "---", ""])

    # Summary
    lines.append("## Summary")
    lines.append("")
    lines.append(f"- Errors: {len(report.errors)}")
    lines.append(f"- Warnings: {len(report.warnings)}")
    lines.append(f"- Info: {len(report.info)}")

    lines.extend(["", "## Recommendation", ""])

    if report.errors:
        lines.append("Fix the errors before archiving:")
        lines.append("- Update proposal to match implementation, OR")
        lines.append("- Implement the missing features")
    else:
        lines.append("No blocking issues. Review warnings and proceed if acceptable.")

    return "\n".join(lines)


def save_report(report: DeviationReport, test_reports_dir: str) -> str:
    """Save deviation report to test-reports directory."""
    os.makedirs(test_reports_dir, exist_ok=True)

    timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
    filename = f"deviation-check-{timestamp}.md"
    filepath = os.path.join(test_reports_dir, filename)

    content = generate_report_markdown(report)
    with open(filepath, "w", encoding="utf-8") as f:
        f.write(content)

    return filepath


# ─── Integration with apply/archive ────────────────────────────────────

def should_block_on_deviation(report: DeviationReport, config: dict) -> bool:
    """Determine if deviation should block archive.

    Args:
        report: DeviationReport from check_deviations.
        config: Configuration dict.

    Returns:
        True if archive should be blocked.
    """
    # Config can override blocking behavior
    if config.get("deviation_block", True) is False:
        return False

    # Block if any errors
    return len(report.errors) > 0


def get_deviation_prompt(report: DeviationReport) -> str:
    """Generate a prompt for Claude to handle deviations.

    Used during apply-change to guide scope alignment.
    """
    if not report.deviations:
        return ""

    lines = [
        "**Scope Deviation Detected**",
        "",
        "The implementation deviates from the proposal scope. Review and address:",
        "",
    ]

    for d in report.errors[:5]:  # Limit to first 5
        if d.type == "missing":
            lines.append(f"- [ ] **Missing**: {d.feature} — implement this feature")
        elif d.type == "extra":
            lines.append(f"- [ ] **Extra**: {d.feature} — consider if this should be in proposal")

    if len(report.errors) > 5:
        lines.append(f"- ... and {len(report.errors) - 5} more")

    lines.extend(["", "Options:", "1. Implement missing features", "2. Update proposal to reflect actual scope"])

    return "\n".join(lines)


# ─── CLI interface ──────────────────────────────────────────────────────

def main():
    import argparse

    parser = argparse.ArgumentParser(description="Deviation check between proposal and implementation")
    parser.add_argument("--change", required=True, help="Change name")
    parser.add_argument("--project-root", default=".", help="Project root directory")
    parser.add_argument("--json", action="store_true", help="Output as JSON")
    parser.add_argument("--save", action="store_true", help="Save report to test-reports/")
    parser.add_argument("--alert", action="store_true", help="Output inline alert only")

    args = parser.parse_args()

    project_root = os.path.abspath(args.project_root)
    change_dir = os.path.join(project_root, "openspec", "changes", args.change)

    if not os.path.isdir(change_dir):
        print(f"Error: Change directory not found: {change_dir}", file=sys.stderr)
        sys.exit(1)

    # Run check
    report = check_deviations(change_dir, project_root)

    if args.alert:
        alert = format_inline_alert(report)
        if alert:
            print(alert)
        sys.exit(0 if report.passed else 1)

    if args.json:
        output = {
            "change_name": report.change_name,
            "date": report.date,
            "passed": report.passed,
            "deviations": [
                {
                    "type": d.type,
                    "severity": d.severity,
                    "feature": d.feature,
                    "details": d.details,
                    "evidence": d.evidence,
                }
                for d in report.deviations
            ],
            "errors": len(report.errors),
            "warnings": len(report.warnings),
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
