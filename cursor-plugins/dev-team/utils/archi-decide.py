#!/usr/bin/env python3
"""
archi-decide.py — Architecture Decision Record (ADR) management utility.

Commands:
  create    Create a new ADR
  list      List/query ADRs
  update    Update ADR status

Usage:
  python archi-decide.py create --title "..." --background "..." --decision "..." ...
  python archi-decide.py list [--status accepted]
  python archi-decide.py update --file "..." --status "accepted"
"""

import argparse
import json
import os
import re
import sys
from datetime import datetime


DECISIONS_DIR = os.path.join("openspec", "architecture", "decisions")
TEMPLATE_PATH = os.path.join("plugins", "dev-team", "templates", "adr.md")

VALID_STATUSES = ("proposed", "accepted", "deprecated", "superseded")


def resolve_project_root():
    """Find project root by locating openspec/ or .git."""
    cwd = os.getcwd()
    for start in [cwd] + list(_walk_up(cwd)):
        if os.path.isdir(os.path.join(start, "openspec")):
            return start
        if os.path.isdir(os.path.join(start, ".git")):
            return start
    return cwd


def _walk_up(path):
    while True:
        parent = os.path.dirname(path)
        if parent == path:
            break
        yield parent
        path = parent


def _slugify(title):
    """Convert a title to kebab-case for filename use."""
    slug = title.lower().strip()
    slug = re.sub(r'[^\w\s-]', '', slug)
    slug = re.sub(r'[\s_]+', '-', slug)
    slug = re.sub(r'-+', '-', slug)
    return slug.strip('-')


def create_adr(project_root, title, background, decision, consequences, alternatives, scope, status="proposed"):
    """Create a new ADR file.

    Args:
        project_root: Project root directory
        title: ADR title
        background: Background context
        decision: The decision
        consequences: Consequences text
        alternatives: List of alternative dicts [{"name":..., "pros":..., "cons":...}]
        scope: Comma-separated model element IDs
        status: Initial status (default: proposed)

    Returns:
        dict with result info
    """
    if status not in VALID_STATUSES:
        return {"success": False, "error": f"Invalid status '{status}'. Must be one of: {', '.join(VALID_STATUSES)}"}

    date_str = datetime.now().strftime("%Y-%m-%d")
    slug = _slugify(title)
    filename = f"{date_str}-{slug}.md"

    decisions_dir = os.path.join(project_root, DECISIONS_DIR)
    os.makedirs(decisions_dir, exist_ok=True)

    filepath = os.path.join(decisions_dir, filename)

    if os.path.exists(filepath):
        return {"success": False, "error": f"ADR already exists: {filename}"}

    # Build alternatives section
    alt_text = ""
    if isinstance(alternatives, str):
        try:
            alternatives = json.loads(alternatives)
        except json.JSONDecodeError:
            alternatives = []

    if alternatives:
        for i, alt in enumerate(alternatives, 1):
            alt_text += f"\n### 方案 {i}：{alt.get('name', 'Unnamed')}\n"
            alt_text += f"- **描述**: {alt.get('description', alt.get('name', ''))}\n"
            pros = alt.get('pros', [])
            if isinstance(pros, list):
                for p in pros:
                    alt_text += f"- **优点**: {p}\n"
            else:
                alt_text += f"- **优点**: {pros}\n"
            cons = alt.get('cons', [])
            if isinstance(cons, list):
                for c in cons:
                    alt_text += f"- **缺点**: {c}\n"
            else:
                alt_text += f"- **缺点**: {cons}\n"

    # Build scope list
    scope_items = [s.strip() for s in scope.split(",") if s.strip()]
    scope_text = "\n".join(f"- {s}" for s in scope_items) if scope_items else "- (none)"

    content = f"""# ADR: {title}

- **日期**: {date_str}
- **状态**: {status}

## 背景

{background}

## 决策

{decision}

## 后果

### 正面后果
- (见下方描述)

### 负面后果
- (见下方描述)

{consequences}

## 备选方案
{alt_text if alt_text else "\n(无备选方案记录)\n"}

## 影响范围

{scope_text}
"""

    with open(filepath, "w", encoding="utf-8") as f:
        f.write(content)

    return {
        "success": True,
        "path": filepath,
        "filename": filename,
    }


def list_adrs(project_root, status_filter=None):
    """List ADRs, optionally filtered by status.

    Args:
        project_root: Project root directory
        status_filter: Optional status to filter by

    Returns:
        dict with adrs list
    """
    decisions_dir = os.path.join(project_root, DECISIONS_DIR)

    if not os.path.isdir(decisions_dir):
        return {"adrs": [], "count": 0}

    adrs = []
    try:
        for filename in sorted(os.listdir(decisions_dir), reverse=True):
            if not filename.endswith(".md"):
                continue

            filepath = os.path.join(decisions_dir, filename)
            try:
                with open(filepath, "r", encoding="utf-8") as f:
                    content = f.read()
            except OSError:
                continue

            # Extract title
            title_match = re.search(r'^# ADR:\s*(.+)$', content, re.MULTILINE)
            title = title_match.group(1).strip() if title_match else filename

            # Extract status
            status_match = re.search(r'\*\*状态\*\*:\s*(\w+)', content)
            status = status_match.group(1).strip() if status_match else "unknown"

            # Extract date
            date_match = re.search(r'\*\*日期\*\*:\s*(\S+)', content)
            date = date_match.group(1).strip() if date_match else ""

            # Extract scope
            scope_section = re.search(r'## 影响范围\s*\n(.*?)(?=\n##|\Z)', content, re.DOTALL)
            scope = []
            if scope_section:
                scope = [s.strip("- ").strip() for s in scope_section.group(1).strip().split("\n") if s.strip().startswith("-")]

            if status_filter and status != status_filter:
                continue

            adrs.append({
                "filename": filename,
                "title": title,
                "status": status,
                "date": date,
                "scope": scope,
                "path": filepath,
            })

    except OSError:
        pass

    return {"adrs": adrs, "count": len(adrs)}


def update_adr(project_root, filename, status, superseded_by=None):
    """Update an ADR's status.

    Args:
        project_root: Project root directory
        filename: ADR filename (e.g., "2026-05-12-use-postgresql.md")
        status: New status value
        superseded_by: Reference to superseding ADR (required when status=superseded)

    Returns:
        dict with result info
    """
    if status not in VALID_STATUSES:
        return {"success": False, "error": f"Invalid status '{status}'. Must be one of: {', '.join(VALID_STATUSES)}"}

    if status == "superseded" and not superseded_by:
        return {"success": False, "error": "Status 'superseded' requires --superseded-by reference"}

    filepath = os.path.join(project_root, DECISIONS_DIR, filename)

    if not os.path.isfile(filepath):
        return {"success": False, "error": f"ADR not found: {filename}"}

    try:
        with open(filepath, "r", encoding="utf-8") as f:
            content = f.read()
    except OSError as e:
        return {"success": False, "error": f"Failed to read ADR: {e}"}

    # Update status line
    old_status_match = re.search(r'(\*\*状态\*\*:\s*)\w+', content)
    if old_status_match:
        content = content[:old_status_match.start(1)] + f'**状态**: {status}' + content[old_status_match.end():]

        # Add superseded reference if applicable
        if status == "superseded" and superseded_by:
            ref_line = f"\n- **取代者**: {superseded_by}"
            if "**取代者**" not in content:
                # Insert after status line
                status_end = content.find('\n', content.find(f'**状态**: {status}'))
                if status_end > 0:
                    content = content[:status_end] + ref_line + content[status_end:]

    with open(filepath, "w", encoding="utf-8") as f:
        f.write(content)

    return {
        "success": True,
        "path": filepath,
        "old_status": old_status_match.group(1).strip() if old_status_match else "unknown",
        "new_status": status,
    }


def main():
    parser = argparse.ArgumentParser(description="Architecture Decision Record management")
    subparsers = parser.add_subparsers(dest="command", help="Command to execute")

    # create
    create_parser = subparsers.add_parser("create", help="Create a new ADR")
    create_parser.add_argument("--title", required=True, help="ADR title")
    create_parser.add_argument("--background", required=True, help="Background context")
    create_parser.add_argument("--decision", required=True, help="The decision")
    create_parser.add_argument("--consequences", default="", help="Consequences text")
    create_parser.add_argument("--alternatives", default="[]", help="JSON array of alternatives")
    create_parser.add_argument("--scope", default="", help="Comma-separated model element IDs")
    create_parser.add_argument("--status", default="proposed", help="Initial status")

    # list
    list_parser = subparsers.add_parser("list", help="List ADRs")
    list_parser.add_argument("--status", default=None, help="Filter by status")

    # update
    update_parser = subparsers.add_parser("update", help="Update ADR status")
    update_parser.add_argument("--file", required=True, help="ADR filename")
    update_parser.add_argument("--status", required=True, help="New status")
    update_parser.add_argument("--superseded-by", default=None, help="Superseding ADR reference")

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(1)

    project_root = resolve_project_root()

    if args.command == "create":
        result = create_adr(
            project_root,
            args.title,
            args.background,
            args.decision,
            args.consequences,
            args.alternatives,
            args.scope,
            args.status,
        )
    elif args.command == "list":
        result = list_adrs(project_root, args.status)
    elif args.command == "update":
        result = update_adr(project_root, args.file, args.status, args.superseded_by)
    else:
        result = {"success": False, "error": f"Unknown command: {args.command}"}

    print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
