#!/usr/bin/env python3
"""
Hook: SessionStart - Ensure OpenSpec CLI is available.

Checks if openspec CLI is installed. If not, prompts user to install.
No skill sync logic - openspec CLI is the source of truth for skills.
"""

import json
import os
import shutil
import sys


def check_openspec_installed():
    """Check if openspec CLI is available in PATH."""
    return shutil.which("openspec") is not None


def output_result(additional_context):
    """Output the hook result as JSON."""
    result = {
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": additional_context,
        }
    }
    json.dump(result, sys.stdout)


def main():
    # Support standalone check
    if len(sys.argv) > 1 and sys.argv[1] == "--check":
        if check_openspec_installed():
            print("openspec CLI is installed.")
            sys.exit(0)
        else:
            print("openspec CLI is NOT installed.", file=sys.stderr)
            print("Install with: npm install -g openspec-cli", file=sys.stderr)
            sys.exit(1)

    # Hook mode: check and optionally install
    if check_openspec_installed():
        # Already installed - no additional context needed
        output_result("")
        return

    # Not installed - inject prompt context
    context = (
        "OpenSpec CLI is not installed. "
        "This plugin requires openspec for SDD workflow. "
        "Install now? Run: npm install -g openspec-cli"
    )
    output_result(context)


if __name__ == "__main__":
    main()
