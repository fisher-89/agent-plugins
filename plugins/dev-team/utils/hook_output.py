#!/usr/bin/env python3
"""
Shared utility for hook output.

Provides unified output_result function for all hook types:
- PreToolUse hooks: permissionDecision + additionalContext
- SessionStart hooks: continue + systemMessage
"""

import json
import sys
from typing import Optional


def output_pre_tool_use(decision: str, additional_context: str = ""):
    """
    Output result for PreToolUse hooks.

    Args:
        decision: "allow" or "deny"
        additional_context: Optional context message
    """
    result = {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": decision,
        }
    }
    if additional_context:
        result["hookSpecificOutput"]["additionalContext"] = additional_context
    json.dump(result, sys.stdout)


def output_session_start(message: Optional[str] = None, stop: bool = False):
    """
    Output result for SessionStart hooks.

    Args:
        message: Optional system message
        stop: If True, sets stopReason instead of systemMessage

    Note:
        SessionStart hooks must exit 0 for JSON to be processed.
        This function prints JSON and exits.
    """
    result = {
        "continue": not stop,
        "suppressOutput": False,
    }

    if message:
        if stop:
            result["stopReason"] = message
        else:
            result["systemMessage"] = message

    print(json.dumps(result))
    sys.exit(0)


def output_user_prompt_submit(additional_context: str = ""):
    """
    Output result for UserPromptSubmit hooks.

    Args:
        additional_context: Context message for the hook
    """
    result = {
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": additional_context,
        }
    }
    json.dump(result, sys.stdout)
