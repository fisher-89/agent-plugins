#!/usr/bin/env python3
"""
Hook: SessionStart - Ensure OpenSpec CLI is available and initialized.

Checks:
1. If openspec CLI is installed
"""

import json
import shutil
import sys


def check_openspec_installed():
    """Check if openspec CLI is available in PATH."""
    return shutil.which("openspec") is not None

def output_result(message: str | None = None, stop: bool = False):
    """
    Output the hook result as JSON.

    JSON is only processed on exit code 0, so we always exit 0 and use additionalContext
    to communicate issues.
    """
    result = {
        "continue": not stop,
        "suppressOutput": False,
        "systemMessage": message,
    }
    print(json.dumps(result))
    sys.exit(0)


def main():
    # Check openspec CLI installed
    if not check_openspec_installed():
        issues = [
            "OpenSpec CLI is not installed. "
            "This plugin requires openspec for SDD workflow. "
            "Install now? Run: npm install -g openspec-cli"
        ]
        output_result(" ".join(issues))
        return

    output_result()


if __name__ == "__main__":
    main()
