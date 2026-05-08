#!/usr/bin/env python3
"""
Review loop state management.

Manages the state of the Post-Review Loop cycle to ensure convergence
and prevent infinite loops.
"""

import json
import os
import sys
from datetime import datetime
from typing import Dict, List, Optional
from dataclasses import dataclass, field, asdict


DEFAULT_MAX_LOOPS = 3
STATE_FILENAME = "review-loop-state.json"


@dataclass
class LoopHistory:
    """History entry for one review loop round."""
    round: int
    errors: int
    tasks_generated: int
    timestamp: str = field(default_factory=lambda: datetime.now().isoformat())


@dataclass
class ReviewLoopState:
    """State of the review loop cycle."""
    change: str
    loop_count: int = 0
    max_loops: int = DEFAULT_MAX_LOOPS
    errors_fixed: int = 0
    current_errors: int = 0
    history: List[LoopHistory] = field(default_factory=list)
    status: str = "idle"  # idle, running, converged, paused
    last_updated: str = field(default_factory=lambda: datetime.now().isoformat())

    def can_continue(self) -> bool:
        """Check if another loop iteration is allowed."""
        return self.loop_count < self.max_loops

    def is_converged(self) -> bool:
        """Check if the loop has converged (no more errors)."""
        return self.current_errors == 0

    def should_pause(self) -> bool:
        """Check if manual intervention is needed."""
        return self.loop_count >= self.max_loops and self.current_errors > 0

    def get_progress_percent(self) -> int:
        """Get progress as percentage (errors fixed vs total)."""
        total = self.errors_fixed + self.current_errors
        if total == 0:
            return 100
        return int((self.errors_fixed / total) * 100)


def get_state_path(change_dir: str) -> str:
    """
    Get the path to the state file for a change.

    Args:
        change_dir: Path to the openspec change directory.

    Returns:
        Path to the state file.
    """
    return os.path.join(change_dir, STATE_FILENAME)


def load_loop_state(change_dir: str) -> ReviewLoopState:
    """
    Load the review loop state for a change.

    Args:
        change_dir: Path to the openspec change directory.

    Returns:
        ReviewLoopState, or a new state if none exists.
    """
    state_path = get_state_path(change_dir)

    if not os.path.isfile(state_path):
        # Return new state with change name from directory
        change_name = os.path.basename(change_dir)
        return ReviewLoopState(change=change_name)

    try:
        with open(state_path, 'r', encoding='utf-8') as f:
            data = json.load(f)

        # Convert history entries
        history = [
            LoopHistory(**h) for h in data.get('history', [])
        ]

        return ReviewLoopState(
            change=data.get('change', ''),
            loop_count=data.get('loop_count', 0),
            max_loops=data.get('max_loops', DEFAULT_MAX_LOOPS),
            errors_fixed=data.get('errors_fixed', 0),
            current_errors=data.get('current_errors', 0),
            history=history,
            status=data.get('status', 'idle'),
            last_updated=data.get('last_updated', ''),
        )
    except (json.JSONDecodeError, KeyError) as e:
        # Return new state on error
        change_name = os.path.basename(change_dir)
        return ReviewLoopState(change=change_name)


def save_loop_state(change_dir: str, state: ReviewLoopState) -> None:
    """
    Save the review loop state to disk.

    Args:
        change_dir: Path to the openspec change directory.
        state: The state to save.
    """
    state_path = get_state_path(change_dir)
    state.last_updated = datetime.now().isoformat()

    # Convert to dict for JSON serialization
    data = {
        'change': state.change,
        'loop_count': state.loop_count,
        'max_loops': state.max_loops,
        'errors_fixed': state.errors_fixed,
        'current_errors': state.current_errors,
        'history': [asdict(h) for h in state.history],
        'status': state.status,
        'last_updated': state.last_updated,
    }

    with open(state_path, 'w', encoding='utf-8') as f:
        json.dump(data, f, indent=2)


def start_loop(change_dir: str) -> ReviewLoopState:
    """
    Start a new review loop cycle.

    Args:
        change_dir: Path to the openspec change directory.

    Returns:
        Updated state with status 'running'.
    """
    state = load_loop_state(change_dir)
    state.status = 'running'
    save_loop_state(change_dir, state)
    return state


def record_review_result(
    change_dir: str,
    error_count: int,
    tasks_generated: int
) -> ReviewLoopState:
    """
    Record the result of a review iteration.

    Args:
        change_dir: Path to the openspec change directory.
        error_count: Number of errors found in this review.
        tasks_generated: Number of fix tasks generated.

    Returns:
        Updated state.
    """
    state = load_loop_state(change_dir)

    # Increment loop count
    state.loop_count += 1

    # Record history
    history_entry = LoopHistory(
        round=state.loop_count,
        errors=error_count,
        tasks_generated=tasks_generated,
    )
    state.history.append(history_entry)

    # Update counters
    if tasks_generated > 0:
        # Fix tasks were generated
        state.current_errors = error_count
    else:
        # No errors, converged
        state.errors_fixed += state.current_errors
        state.current_errors = 0

    # Update status
    if state.is_converged():
        state.status = 'converged'
    elif state.should_pause():
        state.status = 'paused'
    else:
        state.status = 'running'

    save_loop_state(change_dir, state)
    return state


def increment_loop(change_dir: str) -> ReviewLoopState:
    """
    Increment the loop counter (after fix tasks are complete).

    Args:
        change_dir: Path to the openspec change directory.

    Returns:
        Updated state.
    """
    state = load_loop_state(change_dir)
    state.status = 'running'
    save_loop_state(change_dir, state)
    return state


def check_loop_convergence(change_dir: str) -> Dict:
    """
    Check if the loop has converged or needs manual intervention.

    Args:
        change_dir: Path to the openspec change directory.

    Returns:
        Dict with 'action' and optional 'message':
        - action: 'continue', 'converged', 'pause'
        - message: Explanation of the action
    """
    state = load_loop_state(change_dir)

    if state.is_converged():
        return {
            'action': 'converged',
            'message': f"Review loop converged after {state.loop_count} round(s). All errors fixed.",
            'state': state,
        }

    if state.should_pause():
        return {
            'action': 'pause',
            'message': (
                f"Review loop reached maximum iterations ({state.max_loops}). "
                f"Still have {state.current_errors} error(s) remaining. "
                "Manual intervention required."
            ),
            'state': state,
        }

    return {
        'action': 'continue',
        'message': f"Loop {state.loop_count}/{state.max_loops}. {state.current_errors} error(s) remaining.",
        'state': state,
    }


def reset_loop_state(change_dir: str) -> ReviewLoopState:
    """
    Reset the loop state (e.g., after archiving).

    Args:
        change_dir: Path to the openspec change directory.

    Returns:
        Fresh state.
    """
    state_path = get_state_path(change_dir)

    # Delete existing state file
    if os.path.isfile(state_path):
        os.remove(state_path)

    # Return new state
    change_name = os.path.basename(change_dir)
    return ReviewLoopState(change=change_name)


def get_summary(change_dir: str) -> str:
    """
    Get a human-readable summary of the loop state.

    Args:
        change_dir: Path to the openspec change directory.

    Returns:
        Summary string.
    """
    state = load_loop_state(change_dir)

    lines = [
        f"## Review Loop State: {state.change}",
        "",
        f"**Status**: {state.status}",
        f"**Loop**: {state.loop_count}/{state.max_loops}",
        f"**Errors Fixed**: {state.errors_fixed}",
        f"**Current Errors**: {state.current_errors}",
        f"**Progress**: {state.get_progress_percent()}%",
        "",
    ]

    if state.history:
        lines.append("### History")
        lines.append("")
        lines.append("| Round | Errors | Tasks Generated | Timestamp |")
        lines.append("|-------|--------|-----------------|-----------|")
        for h in state.history:
            lines.append(f"| {h.round} | {h.errors} | {h.tasks_generated} | {h.timestamp} |")
        lines.append("")

    if state.should_pause():
        lines.append("⚠️ **Manual intervention required**: Maximum loop iterations reached.")
    elif state.is_converged():
        lines.append("✅ **Converged**: All errors fixed.")
    else:
        remaining = state.max_loops - state.loop_count
        lines.append(f"ℹ️ **{remaining} iteration(s) remaining**")

    return "\n".join(lines)


# CLI interface
if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Manage review loop state")
    parser.add_argument("change_dir", help="Path to openspec change directory")
    parser.add_argument("--start", action="store_true", help="Start a new loop")
    parser.add_argument("--record", nargs=2, metavar=('ERRORS', 'TASKS'),
                        help="Record review result (error_count, tasks_generated)")
    parser.add_argument("--check", action="store_true", help="Check convergence")
    parser.add_argument("--reset", action="store_true", help="Reset state")
    parser.add_argument("--summary", action="store_true", help="Show summary")
    parser.add_argument("--json", action="store_true", help="Output as JSON")

    args = parser.parse_args()

    if args.start:
        state = start_loop(args.change_dir)
        print(f"Started review loop for: {state.change}")

    elif args.record:
        error_count = int(args.record[0])
        tasks_generated = int(args.record[1])
        state = record_review_result(args.change_dir, error_count, tasks_generated)
        print(f"Recorded: {error_count} errors, {tasks_generated} tasks generated")
        print(f"Loop {state.loop_count}/{state.max_loops}, Status: {state.status}")

    elif args.check:
        result = check_loop_convergence(args.change_dir)
        print(f"Action: {result['action']}")
        print(result['message'])

    elif args.reset:
        state = reset_loop_state(args.change_dir)
        print(f"Reset state for: {state.change}")

    elif args.summary:
        print(get_summary(args.change_dir))

    else:
        # Default: show current state
        state = load_loop_state(args.change_dir)

        if args.json:
            print(json.dumps({
                'change': state.change,
                'loop_count': state.loop_count,
                'max_loops': state.max_loops,
                'errors_fixed': state.errors_fixed,
                'current_errors': state.current_errors,
                'status': state.status,
                'last_updated': state.last_updated,
                'history': [asdict(h) for h in state.history],
            }, indent=2))
        else:
            print(f"Change: {state.change}")
            print(f"Status: {state.status}")
            print(f"Loop: {state.loop_count}/{state.max_loops}")
            print(f"Errors fixed: {state.errors_fixed}")
            print(f"Current errors: {state.current_errors}")
            print(f"Can continue: {state.can_continue()}")
            print(f"Is converged: {state.is_converged()}")
            print(f"Should pause: {state.should_pause()}")
