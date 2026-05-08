#!/usr/bin/env python3
"""
Code review result parser.

Parses code review reports to extract ERROR entries and generate fix tasks.
"""

import os
import re
import sys
from datetime import datetime
from typing import List, Optional, Dict
from dataclasses import dataclass, field


@dataclass
class ReviewFinding:
    """A single finding from code review."""
    level: str  # ERROR, WARN, INFO
    file: str
    line: int
    category: str
    description: str
    current_code: Optional[str] = None
    suggested_fix: Optional[str] = None
    suggestion: Optional[str] = None  # For WARN


@dataclass
class ReviewResult:
    """Parsed code review result."""
    change_name: str
    date: str
    verdict: str  # PASS, BLOCK
    staged_files: List[str] = field(default_factory=list)
    findings: List[ReviewFinding] = field(default_factory=list)
    error_count: int = 0
    warn_count: int = 0
    info_count: int = 0

    def has_errors(self) -> bool:
        return self.error_count > 0

    def get_errors(self) -> List[ReviewFinding]:
        return [f for f in self.findings if f.level == "ERROR"]

    def get_warnings(self) -> List[ReviewFinding]:
        return [f for f in self.findings if f.level == "WARN"]


@dataclass
class FixTask:
    """A fix task generated from an ERROR finding."""
    description: str
    file: str
    line: int
    category: str
    current_code: Optional[str]
    suggested_fix: Optional[str]
    review_ref: str  # Reference to original review report


def parse_review_report(report_path: str) -> ReviewResult:
    """
    Parse a code review report file.

    Args:
        report_path: Path to the code review report markdown file.

    Returns:
        ReviewResult with parsed findings.
    """
    with open(report_path, 'r', encoding='utf-8') as f:
        content = f.read()

    return parse_review_content(content, report_path)


def parse_review_content(content: str, report_path: str = "") -> ReviewResult:
    """
    Parse code review report content.

    Args:
        content: The markdown content of the report.
        report_path: Optional path for reference.

    Returns:
        ReviewResult with parsed findings.
    """
    result = ReviewResult(
        change_name="",
        date="",
        verdict=""
    )

    # Parse header
    # > **Change**: <change-name>
    match = re.search(r'\*\*Change\*\*:\s*(.+)', content)
    if match:
        result.change_name = match.group(1).strip()

    # > **Date**: <YYYY-MM-DD HH:MM>
    match = re.search(r'\*\*Date\*\*:\s*(.+)', content)
    if match:
        result.date = match.group(1).strip()

    # > **Verdict**: PASS / BLOCK
    match = re.search(r'\*\*Verdict\*\*:\s*(PASS|BLOCK)', content)
    if match:
        result.verdict = match.group(1)

    # Parse staged files
    staged_match = re.search(r'\*\*Staged Files\*\*:\s*\n((?:-\s+.+\n?)+)', content)
    if staged_match:
        files_text = staged_match.group(1)
        result.staged_files = re.findall(r'-\s+(.+)', files_text)

    # Parse ERROR findings
    # #### [ERROR-N] <file>:<line>
    # - **Category**: <category>
    # - **Description**: <description>
    # - **Current Code**: ...
    # - **Suggested Fix**: ...
    error_pattern = r'####\s*\[ERROR-\d+\]\s*(.+?):(\d+)\s*\n((?:-\s*\*\*[^*]+\*\*:.+\n?)+)'
    for match in re.finditer(error_pattern, content):
        file_path = match.group(1).strip()
        line = int(match.group(2))
        details_block = match.group(3)

        finding = ReviewFinding(
            level="ERROR",
            file=file_path,
            line=line,
            category=_extract_field(details_block, "Category"),
            description=_extract_field(details_block, "Description"),
            current_code=_extract_code_block(details_block, "Current Code"),
            suggested_fix=_extract_code_block(details_block, "Suggested Fix"),
        )
        result.findings.append(finding)
        result.error_count += 1

    # Parse WARN findings
    # #### [WARN-N] <file>:<line>
    # - **Category**: <category>
    # - **Description**: <description>
    # - **Suggestion**: <suggestion>
    warn_pattern = r'####\s*\[WARN-\d+\]\s*(.+?):(\d+)\s*\n((?:-\s*\*\*[^*]+\*\*:.+\n?)+)'
    for match in re.finditer(warn_pattern, content):
        file_path = match.group(1).strip()
        line = int(match.group(2))
        details_block = match.group(3)

        finding = ReviewFinding(
            level="WARN",
            file=file_path,
            line=line,
            category=_extract_field(details_block, "Category"),
            description=_extract_field(details_block, "Description"),
            suggestion=_extract_field(details_block, "Suggestion"),
        )
        result.findings.append(finding)
        result.warn_count += 1

    # Parse INFO findings (simpler format)
    # #### [INFO-N]
    # - <observation>
    info_pattern = r'####\s*\[INFO-\d+\]\s*\n-\s*(.+)'
    for match in re.finditer(info_pattern, content):
        finding = ReviewFinding(
            level="INFO",
            file="",
            line=0,
            category="Info",
            description=match.group(1).strip(),
        )
        result.findings.append(finding)
        result.info_count += 1

    # Alternative parsing for simpler ERROR format (if structured format not found)
    if result.error_count == 0:
        # Try simpler format: [ERROR] file:line - description
        simple_error_pattern = r'\[ERROR\]\s*(.+?):(\d+)\s*[-:]?\s*(.+)'
        for match in re.finditer(simple_error_pattern, content):
            finding = ReviewFinding(
                level="ERROR",
                file=match.group(1).strip(),
                line=int(match.group(2)),
                category="Unknown",
                description=match.group(3).strip(),
            )
            result.findings.append(finding)
            result.error_count += 1

    return result


def _extract_field(block: str, field_name: str) -> str:
    """Extract a field value from a details block."""
    pattern = rf'\*\*{field_name}\*\*:\s*(.+)'
    match = re.search(pattern, block)
    if match:
        return match.group(1).strip()
    return ""


def _extract_code_block(block: str, field_name: str) -> Optional[str]:
    """Extract a code block following a field."""
    # Pattern: **Field**: followed by ``` ... ```
    pattern = rf'\*\*{field_name}\*\*:\s*\n```\s*\n(.+?)\n```'
    match = re.search(pattern, block, re.DOTALL)
    if match:
        return match.group(1).strip()

    # Alternative: inline code
    pattern = rf'\*\*{field_name}\*\*:\s*`(.+?)`'
    match = re.search(pattern, block)
    if match:
        return match.group(1).strip()

    return None


def generate_fix_task(
    error: ReviewFinding,
    review_path: str,
    task_index: int
) -> FixTask:
    """
    Generate a fix task from an ERROR finding.

    Args:
        error: The ERROR finding.
        review_path: Path to the original review report.
        task_index: Index for the task (for numbering).

    Returns:
        FixTask with description and details.
    """
    # Generate short description from error description
    short_desc = error.description
    if len(short_desc) > 60:
        short_desc = short_desc[:57] + "..."

    return FixTask(
        description=f"Fix: {short_desc} ({error.file}:{error.line})",
        file=error.file,
        line=error.line,
        category=error.category,
        current_code=error.current_code,
        suggested_fix=error.suggested_fix,
        review_ref=review_path,
    )


def generate_fix_task_markdown(
    error: ReviewFinding,
    review_path: str,
    task_index: int
) -> str:
    """
    Generate a fix task in markdown format for tasks.md.

    Args:
        error: The ERROR finding.
        review_path: Path to the original review report.
        task_index: Index for the task.

    Returns:
        Markdown string for the fix task.
    """
    fix_task = generate_fix_task(error, review_path, task_index)

    lines = [
        f"- [ ] {fix_task.description}",
        f"  - **Category**: {fix_task.category}",
        f"  - **Location**: `{fix_task.file}:{fix_task.line}`",
    ]

    if fix_task.current_code:
        # Truncate long code
        code = fix_task.current_code
        if len(code) > 200:
            code = code[:200] + "..."
        lines.append(f"  - **Current**: `{code}`")

    if fix_task.suggested_fix:
        code = fix_task.suggested_fix
        if len(code) > 200:
            code = code[:200] + "..."
        lines.append(f"  - **Suggested**: `{code}`")

    lines.append(f"  - **Review**: {fix_task.review_ref}")

    return "\n".join(lines)


def generate_all_fix_tasks(
    review_path: str
) -> List[str]:
    """
    Parse a review report and generate fix tasks for all ERRORs.

    Args:
        review_path: Path to the code review report.

    Returns:
        List of markdown strings for fix tasks.
    """
    result = parse_review_report(review_path)
    errors = result.get_errors()

    tasks = []
    for i, error in enumerate(errors, 1):
        task_md = generate_fix_task_markdown(error, review_path, i)
        tasks.append(task_md)

    return tasks


def append_fix_tasks_to_tasks_md(
    tasks_md_path: str,
    review_path: str
) -> int:
    """
    Append fix tasks to a tasks.md file.

    Args:
        tasks_md_path: Path to the tasks.md file.
        review_path: Path to the code review report.

    Returns:
        Number of fix tasks appended.
    """
    tasks = generate_all_fix_tasks(review_path)

    if not tasks:
        return 0

    # Read existing content
    with open(tasks_md_path, 'r', encoding='utf-8') as f:
        content = f.read()

    # Append fix tasks section
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M")
    new_section = f"\n\n---\n\n## Fix Tasks (from Code Review)\n\n> Generated: {timestamp}\n\n"
    new_section += "\n\n".join(tasks)

    # Write back
    with open(tasks_md_path, 'w', encoding='utf-8') as f:
        f.write(content + new_section)

    return len(tasks)


# CLI interface
if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Parse code review reports")
    parser.add_argument("report_path", help="Path to code review report")
    parser.add_argument("--generate-tasks", action="store_true",
                        help="Generate fix tasks")
    parser.add_argument("--append-to", help="Append fix tasks to tasks.md file")
    parser.add_argument("--json", action="store_true", help="Output as JSON")

    args = parser.parse_args()

    if args.generate_tasks or args.append_to:
        tasks = generate_all_fix_tasks(args.report_path)

        if args.append_to:
            count = append_fix_tasks_to_tasks_md(args.append_to, args.report_path)
            print(f"Appended {count} fix tasks to {args.append_to}")
        else:
            for task in tasks:
                print(task)
                print()
    else:
        result = parse_review_report(args.report_path)

        if args.json:
            import json
            print(json.dumps({
                "change_name": result.change_name,
                "date": result.date,
                "verdict": result.verdict,
                "staged_files": result.staged_files,
                "error_count": result.error_count,
                "warn_count": result.warn_count,
                "info_count": result.info_count,
                "findings": [
                    {
                        "level": f.level,
                        "file": f.file,
                        "line": f.line,
                        "category": f.category,
                        "description": f.description,
                    }
                    for f in result.findings
                ]
            }, indent=2))
        else:
            print(f"Code Review: {result.change_name}")
            print(f"Date: {result.date}")
            print(f"Verdict: {result.verdict}")
            print(f"Staged files: {len(result.staged_files)}")
            print(f"Errors: {result.error_count}")
            print(f"Warnings: {result.warn_count}")
            print(f"Info: {result.info_count}")
            print()

            if result.findings:
                print("Findings:")
                for f in result.findings:
                    print(f"  [{f.level}] {f.file}:{f.line}")
                    print(f"    Category: {f.category}")
                    print(f"    Description: {f.description}")
                    print()
