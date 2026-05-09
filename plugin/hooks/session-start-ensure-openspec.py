#!/usr/bin/env python3
"""
Hook: SessionStart - Ensure OpenSpec CLI is available and initialized.

Checks:
1. If openspec CLI is installed
"""

import json
import os
import shutil
import sys

# Import shared hook output utility
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PLUGIN_ROOT = os.path.dirname(SCRIPT_DIR)
UTILS_DIR = os.path.join(PLUGIN_ROOT, "utils")

if UTILS_DIR not in sys.path:
    sys.path.insert(0, UTILS_DIR)

from hook_output import output_session_start


def check_openspec_installed():
    """Check if openspec CLI is available in PATH."""
    return shutil.which("openspec") is not None


def main():
    # Check openspec CLI installed
    if not check_openspec_installed():
        issues = [
            "OpenSpec CLI is not installed. "
            "This plugin requires openspec for SDD workflow. "
            "Install now? Run: npm install -g openspec-cli"
        ]
        output_session_start(" ".join(issues))
        return

    output_session_start()


if __name__ == "__main__":
    main()
