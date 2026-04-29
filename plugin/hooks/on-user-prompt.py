#!/usr/bin/env python3
"""
Hook: UserPromptSubmit - Check and update openspec change documents before processing.
Detects active openspec changes and provides context to Claude for reviewing/updating documents.
"""

import json
import os
import re
import sys


def main():
    # Read JSON input from stdin
    input_data = json.load(sys.stdin)
    prompt = input_data.get("prompt", "")
    cwd = input_data.get("cwd", "")

    changes_dir = os.path.join(cwd, "openspec", "changes")

    # No changes directory or it doesn't exist
    if not os.path.isdir(changes_dir):
        output_result("")
        return

    # Find active changes (not in archive)
    active_changes = []
    try:
        for entry in os.listdir(changes_dir):
            entry_path = os.path.join(changes_dir, entry)
            if os.path.isdir(entry_path) and entry != "archive":
                active_changes.append((entry, entry_path))
    except OSError:
        output_result("")
        return

    if not active_changes:
        output_result("")
        return

    # Build context about existing changes
    lines = ["=== OpenSpec Active Changes Detected ===", ""]

    for change_name, change_dir in active_changes:
        lines.append(f"Change: {change_name}")

        # Check for key artifacts
        for artifact in ["proposal.md", "design.md", "tasks.md", "specs"]:
            artifact_path = os.path.join(change_dir, artifact)
            if os.path.isfile(artifact_path):
                if artifact == "tasks.md":
                    total, done = count_tasks(artifact_path)
                    lines.append(f"  - {artifact}: {done}/{total} complete")
                else:
                    lines.append(f"  - {artifact}: exists")
            elif os.path.isdir(artifact_path):
                lines.append(f"  - {artifact}/: directory")

        # Check schema from .openspec.yaml
        yaml_path = os.path.join(change_dir, ".openspec.yaml")
        if os.path.isfile(yaml_path):
            try:
                with open(yaml_path, "r", encoding="utf-8") as f:
                    for yaml_line in f:
                        m = re.match(r"^schema:\s*(.+)", yaml_line)
                        if m:
                            lines.append(f"  - schema: {m.group(1).strip()}")
                            break
            except OSError:
                pass
        lines.append("")

    lines.extend([
        "=== Instructions ===",
        "Before responding to the user's question:",
        "1. Check if the question relates to an existing change",
        "2. If so, read the relevant change documents (proposal.md, design.md, tasks.md)",
        "3. Determine if the change documents need updates based on the question",
        "4. If updates are needed, suggest them to the user before proceeding",
        "",
    ])

    output_result("\n".join(lines))


def count_tasks(tasks_path):
    """Count total and completed tasks in a tasks.md file."""
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


def output_result(additional_context):
    """Output the hook result as JSON."""
    result = {
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": additional_context,
        }
    }
    json.dump(result, sys.stdout)


if __name__ == "__main__":
    main()