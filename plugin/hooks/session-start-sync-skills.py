#!/usr/bin/env python3
"""
Hook: SessionStart - Auto-sync OpenSpec skills if missing.

Detects if plugin/skills/openspec-* directories exist.
If missing, runs the sync logic to generate them from OpenSpec CLI.
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


# OpenSpec skill names to sync
OPENSPEC_SKILLS = [
    "openspec-explore",
    "openspec-propose",
    "openspec-apply-change",
    "openspec-archive-change",
]


def get_plugin_root():
    """Get plugin root directory."""
    plugin_root = os.environ.get("CLAUDE_PLUGIN_ROOT", "")
    if not plugin_root:
        # Fallback: derive from this script's location
        plugin_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    return plugin_root


def check_openspec_skills_exist(skills_dir):
    """Check if any openspec skills exist in the skills directory."""
    try:
        for entry in os.listdir(skills_dir):
            if entry.startswith("openspec-"):
                return True
    except OSError:
        pass
    return False


def check_openspec_installed():
    """Check if openspec CLI is available in PATH."""
    return shutil.which("openspec") is not None


def install_openspec_cli():
    """
    Attempt to install openspec CLI via npm.

    Returns:
        tuple: (success: bool, message: str)
    """
    # Check if npm is available
    if shutil.which("npm") is None:
        return False, "npm not found. Please install Node.js first."

    try:
        print("Installing openspec CLI via npm...")
        result = subprocess.run(
            ["npm", "install", "-g", "openspec-cli"],
            capture_output=True,
            text=True,
            timeout=120,
        )

        if result.returncode != 0:
            return False, f"npm install failed: {result.stderr.strip()}"

        # Verify installation
        if check_openspec_installed():
            return True, "openspec CLI installed successfully."
        else:
            return False, "Installation succeeded but 'openspec' still not in PATH. You may need to restart your shell."

    except subprocess.TimeoutExpired:
        return False, "Timeout during npm install."
    except Exception as e:
        return False, f"Error during installation: {e}"


def sync_openspec_skills(plugin_root):
    """
    Sync OpenSpec skills from CLI to plugin directory.

    Returns:
        tuple: (success: bool, message: str)
    """
    skills_dir = os.path.join(plugin_root, "skills")

    # 0. Check and auto-install openspec CLI if missing
    if not check_openspec_installed():
        print("'openspec' CLI not found. Attempting auto-install...")
        success, message = install_openspec_cli()
        if not success:
            return False, f"Auto-install failed: {message}"

    # 1. Remove old OpenSpec skills (preserve custom skills like code-review)
    for skill in OPENSPEC_SKILLS:
        skill_path = os.path.join(skills_dir, skill)
        if os.path.isdir(skill_path):
            shutil.rmtree(skill_path)

    # 2. Create temp directory and run openspec init
    with tempfile.TemporaryDirectory() as temp_dir:
        try:
            print(f"openspec init in {temp_dir} {os.path.exists(temp_dir)}")
            # Run openspec init with claude tool (non-interactive)
            # Provide empty input for any prompts
            # Use shell=True on Windows to properly resolve PATH
            result = subprocess.run(
                ["openspec", "init", "--tools", "claude"],
                cwd=temp_dir,
                input="\n",  # Empty input for prompts
                capture_output=True,
                text=True,
                timeout=60,
                shell=True,
            )

            print("openspec inited")

            if result.returncode != 0:
                return False, "Failed to run 'openspec init --tools claude'. Make sure openspec CLI is installed."

            # 3. Copy generated skills to plugin directory
            temp_skills_dir = os.path.join(temp_dir, ".claude", "skills")
            synced_count = 0

            for skill in OPENSPEC_SKILLS:
                skill_source = os.path.join(temp_skills_dir, skill)
                skill_dest = os.path.join(skills_dir, skill)

                if os.path.isdir(skill_source):
                    os.makedirs(skill_dest, exist_ok=True)
                    skill_md = os.path.join(skill_source, "SKILL.md")
                    if os.path.isfile(skill_md):
                        shutil.copy2(skill_md, skill_dest)
                        synced_count += 1

            if synced_count == 0:
                return False, "No OpenSpec skills found in openspec output."

            return True, f"Synced {synced_count}/{len(OPENSPEC_SKILLS)} skills."

        except subprocess.TimeoutExpired:
            return False, "Timeout running openspec init."
        except FileNotFoundError:
            return False, "'openspec' CLI not found. Auto-install may have failed."
        except OSError as e:
            return False, f"Error: {e}"


def main():
    # Support standalone execution with --sync flag
    if len(sys.argv) > 1 and sys.argv[1] == "--sync":
        # Standalone mode: run sync directly
        plugin_root = get_plugin_root()
        skills_dir = os.path.join(plugin_root, "skills")

        print("Syncing OpenSpec skills to", skills_dir + "...")
        success, message = sync_openspec_skills(plugin_root)

        if success:
            print(f"  Synced: {message}")
            print("\nSkills location:", skills_dir)
            print("Run 'git status' to see changes (if tracked).")
            sys.exit(0)
        else:
            print(f"  Error: {message}", file=sys.stderr)
            sys.exit(1)

    # Hook mode: read JSON input
    # input_data = json.load(sys.stdin)
    # cwd is available but not needed for this hook

    plugin_root = get_plugin_root()
    skills_dir = os.path.join(plugin_root, "skills")

    # Check if any openspec skills exist
    if check_openspec_skills_exist(skills_dir):
        output_result("")
        return

    # Skills missing - need to sync
    success, message = sync_openspec_skills(plugin_root)

    if success:
        output_result(
            f"OpenSpec skills auto-synced. {message} "
            "Skills available: openspec-explore, openspec-propose, "
            "openspec-apply-change, openspec-archive-change"
        )
    else:
        output_result(
            f"Failed to sync OpenSpec skills: {message} "
            "Please run manually: npm run sync-skills"
        )


def output_result(additional_context):
    """Output the hook result as JSON."""
    result = {
        "hookSpecificOutput": {
            "hookEventName": "SessionStart",
            "additionalContext": additional_context,
        }
    }
    json.dump(result, sys.stdout)


if __name__ == "__main__":
    main()