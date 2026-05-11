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
from dataclasses import dataclass
from typing import Literal

# Import shared hook output utility
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PLUGIN_ROOT = os.path.dirname(SCRIPT_DIR)
UTILS_DIR = os.path.join(PLUGIN_ROOT, "utils")

if UTILS_DIR not in sys.path:
    sys.path.insert(0, UTILS_DIR)

from hook_output import output_user_prompt_submit


# =============================================================================
# Intent Classification (Tasks 1.1-1.6)
# =============================================================================

@dataclass
class IntentResult:
    """Result of intent classification."""
    intent_type: Literal["explore", "implement", "fix", "archive", "question"]
    confidence: Literal["high", "medium", "low"]
    scope: Literal["trivial", "small", "medium", "large"]
    is_trivial_fix: bool
    matched_keywords: list[str]


class IntentClassifier:
    """Classify user intent from prompt text."""

    # Intent patterns: intent_type -> regex patterns
    PATTERNS: dict[str, list[str]] = {
        "explore": [
            r"分析", r"理解", r"研究", r"查看", r"了解",
            r"\banalyze\b", r"\bunderstand\b", r"\bexplore\b", r"\binvestigate\b"
        ],
        "implement": [
            r"实现", r"添加", r"新增", r"增加", r"开发", r"重构", r"优化", r"改进",
            r"\badd\b", r"\bimplement\b", r"\bcreate\b", r"\bdevelop\b",
            r"\brefactor\b", r"\boptimize\b", r"\bimprove\b"
        ],
        "fix": [
            r"修复", r"解决", r"改正",
            r"\bfix\b", r"\bbug\b", r"\bresolve\b", r"\bpatch\b"
        ],
        "archive": [
            r"归档", r"完成", r"结束",
            r"\barchive\b", r"\bdone\b", r"\bcomplete\b", r"\bfinish\b"
        ],
        "question": [
            r"是什么", r"为什么", r"怎么做", r"如何", r"什么",
            r"\bwhat\b", r"\bwhy\b", r"\bhow\b", r"\bexplain\b"
        ]
    }

    # Scope hints: scope -> hint keywords
    SCOPE_HINTS: dict[str, list[str]] = {
        "trivial": [
            r"typo", r"错别字", r"拼写", r"注释", r"comment",
            r"变量名", r"variable name", r"重命名", r"rename"
        ],
        "small": [
            r"单个", r"single", r"一个函数", r"one function",
            r"小", r"small", r"简单", r"simple"
        ],
        "medium": [
            r"多个", r"multiple", r"几个", r"several",
            r"中", r"medium", r"模块", r"module"
        ],
        "large": [
            r"重构", r"refactor", r"整体", r"entire", r"全部",
            r"大", r"large", r"系统", r"system", r"架构", r"architecture"
        ]
    }

    # Trivial fix hints
    TRIVIAL_FIX_HINTS: list[str] = [
        r"内部", r"private", r"helper", r"工具函数", r"内部方法",
        r"typo", r"错别字", r"拼写错误", r"注释", r"comment"
    ]

    def detect(self, prompt: str) -> IntentResult:
        """
        Detect user intent from prompt text.

        Args:
            prompt: The user's prompt text

        Returns:
            IntentResult with intent_type, confidence, scope, is_trivial_fix, matched_keywords
        """
        # Step 1: Match intent keywords
        matched = {}
        matched_keywords = []
        for intent_type, regex_list in self.PATTERNS.items():
            for pattern in regex_list:
                match = re.search(pattern, prompt, re.IGNORECASE)
                if match:
                    matched[intent_type] = matched.get(intent_type, 0) + 1
                    matched_keywords.append(match.group())

        # Step 2: Determine intent type
        if not matched:
            intent_type = "question"  # Default fallback
            confidence = "low"
        else:
            # Find best match
            intent_type = max(matched, key=matched.get)
            count = matched[intent_type]

            if count >= 2:
                confidence = "high"
            elif count == 1:
                confidence = "medium"
            else:
                confidence = "low"

        # Step 3: Estimate scope
        scope = self._estimate_scope(prompt)

        # Step 4: Detect trivial fix
        is_trivial_fix = self._is_trivial_fix(prompt, scope, intent_type)

        return IntentResult(
            intent_type=intent_type,
            confidence=confidence,
            scope=scope,
            is_trivial_fix=is_trivial_fix,
            matched_keywords=matched_keywords
        )

    def _estimate_scope(self, prompt: str) -> Literal["trivial", "small", "medium", "large"]:
        """Estimate change scope from prompt keywords."""
        # Check scope hints in priority order (large -> medium -> small -> trivial)
        for scope in ["large", "medium", "small", "trivial"]:
            for pattern in self.SCOPE_HINTS[scope]:
                if re.search(pattern, prompt, re.IGNORECASE):
                    return scope  # type: ignore

        # Default to medium (safe middle ground)
        return "medium"

    def _is_trivial_fix(
        self,
        prompt: str,
        scope: str,
        intent_type: str
    ) -> bool:
        """
        Detect if this is a trivial fix that doesn't need workflow.

        Heuristics:
        - scope == "trivial" is always trivial
        - Keywords like "内部", "private", "helper" suggest non-public
        - Must be fix or implement intent
        """
        # Trivial scope is always trivial
        if scope == "trivial":
            return True

        # Check trivial fix hints
        for pattern in self.TRIVIAL_FIX_HINTS:
            if re.search(pattern, prompt, re.IGNORECASE):
                return True

        return False


# =============================================================================
# State Analysis (Tasks 2.1-2.4)
# =============================================================================

@dataclass
class ChangeInfo:
    """Information about an active OpenSpec change."""
    name: str
    path: str
    stage: Literal["proposal", "design", "tasks"]
    tasks_total: int
    tasks_done: int

    @property
    def tasks_complete(self) -> bool:
        """All tasks are complete."""
        return self.tasks_total > 0 and self.tasks_done == self.tasks_total

    @property
    def tasks_started(self) -> bool:
        """At least some tasks are in progress."""
        return self.tasks_done > 0 and self.tasks_done < self.tasks_total


class StateAnalyzer:
    """Analyze the state of OpenSpec changes in the project."""

    def find_active_changes(self, cwd: str) -> list[ChangeInfo]:
        """
        Scan openspec/changes directory for active changes.

        Args:
            cwd: Current working directory

        Returns:
            List of ChangeInfo for each active change
        """
        changes_dir = os.path.join(cwd, "openspec", "changes")
        if not os.path.isdir(changes_dir):
            return []

        active_changes = []
        try:
            for entry in os.listdir(changes_dir):
                entry_path = os.path.join(changes_dir, entry)
                if os.path.isdir(entry_path) and entry != "archive":
                    stage = self.get_change_stage(entry_path)
                    tasks_total, tasks_done = self.get_tasks_progress(entry_path)
                    active_changes.append(ChangeInfo(
                        name=entry,
                        path=entry_path,
                        stage=stage,
                        tasks_total=tasks_total,
                        tasks_done=tasks_done
                    ))
        except OSError:
            pass

        return active_changes

    def get_change_stage(self, change_dir: str) -> Literal["proposal", "design", "tasks"]:
        """
        Determine the current stage of a change.

        Args:
            change_dir: Path to the change directory

        Returns:
            Stage: "tasks" if tasks.md exists, "design" if design.md exists, "proposal" otherwise
        """
        if os.path.isfile(os.path.join(change_dir, "tasks.md")):
            return "tasks"
        if os.path.isfile(os.path.join(change_dir, "design.md")):
            return "design"
        return "proposal"

    def get_tasks_progress(self, change_dir: str) -> tuple[int, int]:
        """
        Count completed vs total tasks.

        Args:
            change_dir: Path to the change directory

        Returns:
            Tuple (total, done)
        """
        tasks_path = os.path.join(change_dir, "tasks.md")
        if not os.path.isfile(tasks_path):
            return 0, 0

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


# =============================================================================
# Routing Decision (Tasks 3.1-3.7)
# =============================================================================

@dataclass
class RouteDecision:
    """Routing decision result."""
    action: str  # "explore", "apply-change", "archive", "direct"
    mode: Literal["auto", "suggest", "direct"]
    reason: str
    options: list[str]


class Router:
    """Make routing decisions based on intent and state."""

    def decide(self, intent: IntentResult, changes: list[ChangeInfo]) -> RouteDecision:
        """
        Make routing decision.

        Args:
            intent: The classified intent
            changes: List of active changes

        Returns:
            RouteDecision with action, mode, reason, options
        """
        # Decision matrix based on intent type
        if intent.intent_type == "question":
            return self._route_question(intent, changes)
        elif intent.intent_type == "explore":
            return self._route_explore(intent, changes)
        elif intent.intent_type == "fix":
            return self._route_fix(intent, changes)
        elif intent.intent_type == "implement":
            return self._route_implement(intent, changes)
        elif intent.intent_type == "archive":
            return self._route_archive(intent, changes)
        else:
            # Fallback to direct
            return RouteDecision(
                action="direct",
                mode="direct",
                reason="Unknown intent type, proceeding with direct response",
                options=[]
            )

    def _route_question(self, intent: IntentResult, changes: list[ChangeInfo]) -> RouteDecision:
        """
        Route question intent.

        Decision matrix:
        - High confidence question → direct
        - Low confidence (unknown intent) + active change → suggest based on state
        - Low confidence + no active change → direct
        """
        # If confidence is low, this is actually an "unknown" intent
        # Check if there are active changes that might be relevant
        if intent.confidence == "low" and changes:
            change = changes[0]
            if change.tasks_complete:
                return RouteDecision(
                    action="archive",
                    mode="suggest",
                    reason=f"Unknown intent but active change '{change.name}' has all tasks complete",
                    options=[
                        f"Archive with /openspec-archive-change (change: {change.name})",
                        "Continue working",
                        "Respond directly"
                    ]
                )
            if change.tasks_started:
                return RouteDecision(
                    action="apply-change",
                    mode="suggest",
                    reason=f"Unknown intent but active change '{change.name}' in progress",
                    options=[
                        f"Continue with /openspec-apply-change (change: {change.name})",
                        "Start new workflow with /openspec-explore",
                        "Respond directly"
                    ]
                )
            # Has change but not started
            return RouteDecision(
                action="apply-change",
                mode="suggest",
                reason=f"Unknown intent with existing change '{change.name}'",
                options=[
                    f"Continue with /openspec-apply-change (change: {change.name})",
                    "Start new workflow with /openspec-explore",
                    "Respond directly"
                ]
            )

        # High/medium confidence question or no active changes → direct
        return RouteDecision(
            action="direct",
            mode="direct",
            reason="Question intent detected - answering directly",
            options=[]
        )

    def _route_explore(self, intent: IntentResult, changes: list[ChangeInfo]) -> RouteDecision:
        """Route explore intent - always direct."""
        return RouteDecision(
            action="direct",
            mode="direct",
            reason="Explore intent detected - proceeding with direct exploration",
            options=[]
        )

    def _route_fix(self, intent: IntentResult, changes: list[ChangeInfo]) -> RouteDecision:
        """
        Route fix intent.

        Decision matrix:
        - Trivial fix + high confidence → auto direct
        - Trivial fix + medium/low confidence → suggest
        - Non-trivial fix + active change → suggest apply-change
        - Non-trivial fix + no active change → suggest explore
        """
        if intent.is_trivial_fix:
            if intent.confidence == "high":
                return RouteDecision(
                    action="direct",
                    mode="auto",
                    reason="Trivial fix detected with high confidence - auto-routing to direct fix",
                    options=[]
                )
            else:
                return RouteDecision(
                    action="direct",
                    mode="suggest",
                    reason="Trivial fix detected but confidence is medium/low",
                    options=[
                        "Fix directly (trivial change)",
                        "Start workflow with /openspec-explore"
                    ]
                )

        # Non-trivial fix
        if changes:
            # Has active change - suggest apply-change
            change = changes[0]  # Use first change
            if change.tasks_started:
                return RouteDecision(
                    action="apply-change",
                    mode="suggest",
                    reason=f"Non-trivial fix with active change '{change.name}' in progress",
                    options=[
                        f"Continue with /openspec-apply-change (change: {change.name})",
                        "Fix directly",
                        "Start new workflow with /openspec-explore"
                    ]
                )

        # No active change or no started tasks
        return RouteDecision(
            action="explore",
            mode="suggest",
            reason="Non-trivial fix without active change - suggesting explore phase",
            options=[
                "Start workflow with /openspec-explore",
                "Fix directly"
            ]
        )

    def _route_implement(self, intent: IntentResult, changes: list[ChangeInfo]) -> RouteDecision:
        """
        Route implement intent.

        Decision matrix:
        - High confidence + no active change → auto explore
        - High confidence + active change with tasks → auto apply-change
        - Medium/low confidence → suggest
        """
        if not changes:
            # No active change
            if intent.confidence == "high" and self._can_auto_explore(intent):
                return RouteDecision(
                    action="explore",
                    mode="auto",
                    reason="High confidence implement intent without active change - auto-routing to explore",
                    options=[]
                )
            else:
                return RouteDecision(
                    action="explore",
                    mode="suggest",
                    reason="Implement intent detected without active change",
                    options=[
                        "Start workflow with /openspec-explore",
                        "Implement directly"
                    ]
                )

        # Has active change
        change = changes[0]
        if change.tasks_complete:
            # All tasks complete - suggest archive
            return RouteDecision(
                action="archive",
                mode="suggest",
                reason=f"Active change '{change.name}' has all tasks complete - consider archiving",
                options=[
                    f"Archive change with /openspec-archive-change (change: {change.name})",
                    "Continue implementing",
                    "Commit changes"
                ]
            )

        if change.tasks_started:
            # Tasks in progress
            if intent.confidence == "high":
                return RouteDecision(
                    action="apply-change",
                    mode="auto",
                    reason=f"High confidence implement intent with active change '{change.name}' in progress",
                    options=[]
                )
            else:
                return RouteDecision(
                    action="apply-change",
                    mode="suggest",
                    reason=f"Active change '{change.name}' in progress",
                    options=[
                        f"Continue with /openspec-apply-change (change: {change.name})",
                        "Start new workflow with /openspec-explore",
                        "Implement directly"
                    ]
                )

        # Has change but not started
        if intent.confidence == "high":
            return RouteDecision(
                action="apply-change",
                mode="auto",
                reason=f"High confidence with existing change '{change.name}'",
                options=[]
            )
        else:
            return RouteDecision(
                action="apply-change",
                mode="suggest",
                reason=f"Existing change '{change.name}' found",
                options=[
                    f"Continue with /openspec-apply-change (change: {change.name})",
                    "Start new workflow with /openspec-explore"
                ]
            )

    def _route_archive(self, intent: IntentResult, changes: list[ChangeInfo]) -> RouteDecision:
        """
        Route archive intent.

        Decision matrix:
        - High confidence + change with complete tasks → auto archive
        - Otherwise → suggest
        """
        if not changes:
            return RouteDecision(
                action="direct",
                mode="direct",
                reason="Archive intent but no active changes found",
                options=[]
            )

        change = changes[0]
        if change.tasks_complete and intent.confidence == "high":
            return RouteDecision(
                action="archive",
                mode="auto",
                reason=f"High confidence archive intent with complete change '{change.name}'",
                options=[]
            )

        if change.tasks_complete:
            return RouteDecision(
                action="archive",
                mode="suggest",
                reason=f"Change '{change.name}' has all tasks complete",
                options=[
                    f"Archive with /openspec-archive-change (change: {change.name})",
                    "Continue working",
                    "Commit first"
                ]
            )

        # Tasks not complete
        return RouteDecision(
            action="apply-change",
            mode="suggest",
            reason=f"Archive intent but change '{change.name}' has incomplete tasks ({change.tasks_done}/{change.tasks_total})",
            options=[
                f"Complete tasks with /openspec-apply-change (change: {change.name})",
                f"Archive anyway with /openspec-archive-change (change: {change.name})"
            ]
        )

    def _can_auto_explore(self, intent: IntentResult) -> bool:
        """Check if auto-explore is appropriate."""
        # Don't auto-explore for large scopes
        if intent.scope == "large":
            return False
        return True


# =============================================================================
# Output Formatting (Tasks 4.1-4.5)
# =============================================================================

class OutputBuilder:
    """Build output context for different routing modes."""

    def auto_route(self, decision: RouteDecision) -> str:
        """
        Format auto-route context.

        Args:
            decision: The routing decision

        Returns:
            Formatted auto-route context string
        """
        # Map action to skill name
        skill_map = {
            "explore": "/openspec-explore",
            "apply-change": "/openspec-apply-change",
            "archive": "/openspec-archive-change",
            "direct": ""
        }

        skill = skill_map.get(decision.action, "")
        lines = []

        if skill:
            lines.extend([
                f"Auto-Route: {skill}",
                "",
                f"Reason: {decision.reason}",
                "",
            ])
        else:
            # Direct action - no skill needed
            lines.append(decision.reason)

        return "\n".join(lines)

    def suggestion(self, intent: IntentResult, changes: list[ChangeInfo], decision: RouteDecision) -> str:
        """
        Format suggestion context with intent info and numbered options.

        Args:
            intent: The classified intent
            changes: List of active changes
            decision: The routing decision

        Returns:
            Formatted suggestion context string
        """
        lines = [
            "=== OpenSpec Routing Suggestion ===",
            "",
            f"Detected intent: **{intent.intent_type}** (confidence: {intent.confidence}, scope: {intent.scope})",
            f"Reason: {decision.reason}",
            "",
        ]

        # Show options
        if decision.options:
            lines.append("**Options:**")
            for i, option in enumerate(decision.options, 1):
                lines.append(f"{i}. {option}")
            lines.append("")

        return "\n".join(lines)

    def direct(self, changes: list[ChangeInfo], cwd: str = "") -> str:
        """
        Format direct output with existing change info and auto-commit hints.

        Preserves existing active change reporting behavior.

        Args:
            changes: List of active changes
            cwd: Current working directory

        Returns:
            Formatted direct context string
        """
        if not changes:
            return ""

        lines = ["=== OpenSpec Active Changes Detected ===", ""]

        for change in changes:
            lines.append(f"Change: {change.name}")

            # Check for key artifacts
            for artifact in ["proposal.md", "design.md", "tasks.md", "specs"]:
                artifact_path = os.path.join(change.path, artifact)
                if os.path.isfile(artifact_path):
                    if artifact == "tasks.md":
                        lines.append(f"  - {artifact}: {change.tasks_done}/{change.tasks_total} complete")
                    else:
                        lines.append(f"  - {artifact}: exists")
                elif os.path.isdir(artifact_path):
                    lines.append(f"  - {artifact}/: directory")

            # Check schema from .openspec.yaml
            yaml_path = os.path.join(change.path, ".openspec.yaml")
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
        for change in changes:
            if change.tasks_complete:
                lines.extend([
                    f"=== Auto-Commit: Change '{change.name}' All Tasks Complete ===",
                    f"All {change.tasks_total} tasks in change '{change.name}' are complete. After confirming implementation is done:",
                    "1. Run `git status --porcelain` to check for uncommitted changes",
                    "2. If changes exist, commit them:",
                    "   ```bash",
                   f"   git add -A && git commit -m \"feat({change.name}): complete implementation\"",
                    "   ```",
                    "3. Check if in a git worktree:",
                    "   ```bash",
                    "   git rev-parse --git-dir && git rev-parse --git-common-dir",
                    "   ```",
                    "   If these differ, you are in a worktree — use ExitWorktree tool with action 'keep' to return to the main directory.",
                    "",
                ])

        return "\n".join(lines)


# =============================================================================
# Main Hook Integration (Tasks 5.1-5.4)
# =============================================================================

def main():
    """Main hook entry point - uses new routing classes."""
    # Read JSON input from stdin
    input_data = json.load(sys.stdin)
    prompt = input_data.get("prompt", "")
    cwd = input_data.get("cwd", "")

    # Step 1: Classify intent
    intent = IntentClassifier().detect(prompt)

    # Step 2: Analyze state
    changes = StateAnalyzer().find_active_changes(cwd)

    # Step 3: Make routing decision
    decision = Router().decide(intent, changes)

    # Step 4: Build output based on mode
    builder = OutputBuilder()

    if decision.mode == "auto":
        output = builder.auto_route(decision)
    elif decision.mode == "suggest":
        output = builder.suggestion(intent, changes, decision)
    else:  # direct
        output = builder.direct(changes, cwd)

    output_user_prompt_submit(output)


if __name__ == "__main__":
    main()