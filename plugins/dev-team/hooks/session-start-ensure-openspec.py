#!/usr/bin/env python3
"""
Hook: SessionStart - Ensure OpenSpec CLI and likec4 are available.

Checks:
1. If openspec CLI is installed
2. If likec4 CLI is available (via npx likec4 --version)
"""

import json
import os
import shutil
import subprocess
import sys

# Import shared hook output utility
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PLUGIN_ROOT = os.path.dirname(SCRIPT_DIR)
UTILS_DIR = os.path.join(PLUGIN_ROOT, "utils")

if UTILS_DIR not in sys.path:
    sys.path.insert(0, UTILS_DIR)

from hook_output import output_session_start


def check_openspec_installed():
    """
    Check if openspec CLI is available in PATH or common npm global install paths.

    On macOS, hook subprocesses may not inherit the full PATH from the user's
    interactive shell, so shutil.which() can miss npm global binaries.
    """
    # First try standard PATH lookup
    if shutil.which("openspec") is not None:
        return True

    # Fallback: check common npm global bin directories on macOS
    home = os.path.expanduser("~")
    common_paths = [
        "/usr/local/bin",                           # Intel Mac, standard Node
        "/opt/homebrew/bin",                        # Apple Silicon, Homebrew Node
        "/opt/local/bin",                           # MacPorts
        os.path.join(home, ".nvm", "versions", "node", "v18", "bin"),
        os.path.join(home, ".nvm", "versions", "node", "v20", "bin"),
        os.path.join(home, ".nvm", "versions", "node", "v22", "bin"),
        os.path.join(home, "local", "bin"),
        os.path.join(home, "bin"),
    ]

    for path in common_paths:
        candidate = os.path.join(path, "openspec")
        if os.path.exists(candidate) and os.access(candidate, os.X_OK):
            return True

    return False


def check_likec4_available():
    """Check if likec4 CLI is available via npx.

    Returns True if `npx likec4 --version` succeeds.
    """
    try:
        result = subprocess.run(
            ["npx", "likec4", "--version"],
            capture_output=True,
            text=True,
            timeout=30,
            shell=True,
        )
        return result.returncode == 0
    except (subprocess.TimeoutExpired, OSError, FileNotFoundError):
        return False


def main():
    issues = []

    # Check openspec CLI installed
    if not check_openspec_installed():
        issues.append(
            "OpenSpec CLI is not installed. "
            "This plugin requires openspec for SDD workflow. "
            "Install now? Run: npm install -g @fission-ai/openspec@latest"
        )

    # Check likec4 CLI available
    if not check_likec4_available():
        issues.append(
            "likec4 CLI is not available. "
            "The architecture workflow (archi-model, archi-validate) requires likec4. "
            "Install now? Run: npm install -g likec4"
        )

    if issues:
        output_session_start(" | ".join(issues), True)
        return

    output_session_start()


if __name__ == "__main__":
    main()
