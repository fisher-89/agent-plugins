#!/usr/bin/env python3
"""
Hook: PreToolUse (Bash) - Unified commit gate router.

Single entry point for all git commit validation. Routes to:
  1. SDD quality gates (report chain → lint → test → code review)
  2. Architecture validation gate (model → import → report)

Decision rules:
  - Any deny → deny with that context
  - All allow → allow with combined context
"""

import json
import os
import re
import sys


SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PLUGIN_ROOT = os.path.dirname(os.path.dirname(SCRIPT_DIR))
UTILS_DIR = os.path.join(PLUGIN_ROOT, "utils")

if UTILS_DIR not in sys.path:
    sys.path.insert(0, UTILS_DIR)

from hook_output import output_pre_tool_use


def is_git_commit_command(command):
    """Check if the command is a git commit."""
    if not command:
        return False
    return bool(re.search(r"\bgit\s+commit\b", command))


def _load_gate(module_name, filename):
    """Load a gate module from the commit-gates directory. Returns run_gate or None."""
    import importlib.util
    filepath = os.path.join(SCRIPT_DIR, filename)
    if not os.path.isfile(filepath):
        return None
    try:
        spec = importlib.util.spec_from_file_location(module_name, filepath)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        return getattr(mod, "run_gate", None)
    except Exception:
        return None


# Suite registry — order matters: SDD quality gates first, then architecture gate
SUITES = [
    ("quality", "quality.py"),
    ("architecture", "architecture.py"),
]


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

    # Run all suites in order
    contexts = []
    for suite_name, filename in SUITES:
        run_gate = _load_gate(suite_name, filename)
        if run_gate is None:
            continue

        try:
            decision, context = run_gate(input_data)
        except Exception:
            # Gate failures are non-blocking
            continue

        if decision == "deny":
            output_pre_tool_use("deny", context)
            return

        if context:
            contexts.append(context)

    # All passed — combine context
    combined = " | ".join(contexts) if contexts else ""
    output_pre_tool_use("allow", combined)


if __name__ == "__main__":
    main()
