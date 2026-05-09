#!/usr/bin/env python3
"""
Hook: UserPromptSubmit - Check and update openspec change documents before processing.
Detects active openspec changes and provides context to Claude for reviewing/updating documents.
Includes intent detection to suggest OpenSpec workflow on first conversation.
"""

import json
import os
import re
import sys

# Import shared hook output utility
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PLUGIN_ROOT = os.path.dirname(SCRIPT_DIR)
UTILS_DIR = os.path.join(PLUGIN_ROOT, "utils")

if UTILS_DIR not in sys.path:
    sys.path.insert(0, UTILS_DIR)

from hook_output import output_user_prompt_submit


def detect_intent(prompt: str) -> dict:
    """
    Detect user intent from prompt text.

    Returns:
        {
            "type": "new_feature" | "fix" | "refactor" | "explore" | "archive" | "none",
            "confidence": "high" | "medium" | "low",
            "keywords": [matched keywords]
        }
    """
    patterns = {
        "new_feature": [
            r"实现", r"添加", r"新增", r"增加", r"开发",
            r"\badd\b", r"\bimplement\b", r"\bcreate\b", r"\bdevelop\b",
            r"\bfeature\b", r"\bnew\b"
        ],
        "fix": [
            r"修复", r"解决", r"改正",
            r"\bfix\b", r"\bbug\b", r"\bresolve\b", r"\bpatch\b"
        ],
        "refactor": [
            r"重构", r"优化", r"改进",
            r"\brefactor\b", r"\boptimize\b", r"\bimprove\b", r"\bclean\b"
        ],
        "explore": [
            r"分析", r"理解", r"研究", r"查看", r"了解",
            r"\banalyze\b", r"\bunderstand\b", r"\bexplore\b", r"\binvestigate\b"
        ],
        "archive": [
            r"归档", r"完成", r"结束",
            r"\barchive\b", r"\bdone\b", r"\bcomplete\b", r"\bfinish\b"
        ]
    }

    matched = {}
    for intent_type, regex_list in patterns.items():
        for pattern in regex_list:
            if re.search(pattern, prompt, re.IGNORECASE):
                matched[intent_type] = matched.get(intent_type, 0) + 1

    if not matched:
        return {"type": "none", "confidence": "low", "keywords": []}

    # Find best match
    best_type = max(matched, key=matched.get)
    count = matched[best_type]

    if count >= 2:
        confidence = "high"
    elif count == 1:
        confidence = "medium"
    else:
        confidence = "low"

    return {"type": best_type, "confidence": confidence, "keywords": list(matched.keys())}


def main():
    # Read JSON input from stdin
    input_data = json.load(sys.stdin)
    prompt = input_data.get("prompt", "")
    cwd = input_data.get("cwd", "")

    changes_dir = os.path.join(cwd, "openspec", "changes")
    openspec_exists = os.path.isdir(changes_dir)

    # Find active changes (not in archive)
    active_changes = []
    if openspec_exists:
        try:
            for entry in os.listdir(changes_dir):
                entry_path = os.path.join(changes_dir, entry)
                if os.path.isdir(entry_path) and entry != "archive":
                    active_changes.append((entry, entry_path))
        except OSError:
            pass

    # === Intent Detection (首轮对话) ===
    # If no active changes and OpenSpec exists, detect intent and suggest workflow
    if not active_changes and openspec_exists:
        intent = detect_intent(prompt)

        if intent["type"] != "none" and intent["confidence"] in ("high", "medium"):
            lines = [
                "=== OpenSpec Workflow Suggestion ===",
                "",
                f"Detected intent: **{intent['type']}** (confidence: {intent['confidence']})",
                "",
                "This appears to be a development task. Would you like to use OpenSpec workflow?",
                "",
                "**Benefits:**",
                "- Structured specification before implementation",
                "- Test-driven development with auto-generated test skeletons",
                "- Automatic code review and compliance checks",
                "- Traceable changes from proposal to archive",
                "",
                "**To start:** Reply with `/openspec-explore` to begin the explore phase.",
                "**To skip:** Just say 'no' or continue with your current approach.",
                "",
            ]
            output_user_prompt_submit("\n".join(lines))
            return

    # No changes directory or no active changes
    if not active_changes:
        output_user_prompt_submit("")
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

    # Check for fully completed changes and inject auto-commit instructions
    for change_name, change_dir in active_changes:
        tasks_path = os.path.join(change_dir, "tasks.md")
        if os.path.isfile(tasks_path):
            total, done = count_tasks(tasks_path)
            if total > 0 and done == total:
                lines.extend([
                    f"=== Auto-Commit: Change '{change_name}' All Tasks Complete ===",
                    f"All {total} tasks in change '{change_name}' are complete. After confirming implementation is done:",
                    "1. Run `git status --porcelain` to check for uncommitted changes",
                    "2. If changes exist, commit them:",
                    "   ```bash",
                   f"   git add -A && git commit -m \"feat({change_name}): complete implementation\"",
                    "   ```",
                    "3. Check if in a git worktree:",
                    "   ```bash",
                    "   git rev-parse --git-dir && git rev-parse --git-common-dir",
                    "   ```",
                    "   If these differ, you are in a worktree — use ExitWorktree tool with action 'keep' to return to the main directory.",
                    "",
                ])

    output_user_prompt_submit("\n".join(lines))


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


if __name__ == "__main__":
    main()