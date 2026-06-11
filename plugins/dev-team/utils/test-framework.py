#!/usr/bin/env python3
"""
Test framework detection utility.

Scans project files to detect the testing framework in use and returns
the appropriate test runner command.
"""

import json
import os
import sys
from typing import Tuple, Optional


def detect_test_framework(project_root: str) -> Tuple[str, str]:
    """
    Detect the testing framework used in a project.

    Args:
        project_root: Path to the project root directory.

    Returns:
        Tuple of (framework_name, runner_command).
        Returns ("unknown", "") if no framework detected.
    """
    # Normalize path
    project_root = os.path.abspath(project_root)

    # Check for Node.js project
    package_json = os.path.join(project_root, "package.json")
    if os.path.isfile(package_json):
        framework, runner = _detect_node_framework(package_json)
        if framework != "unknown":
            return (framework, runner)

    # Check for Rust project
    cargo_toml = os.path.join(project_root, "Cargo.toml")
    if os.path.isfile(cargo_toml):
        return ("cargo-test", "cargo test")

    # Check for Python project
    pyproject_toml = os.path.join(project_root, "pyproject.toml")
    if os.path.isfile(pyproject_toml):
        return ("pytest", "pytest")

    requirements_txt = os.path.join(project_root, "requirements.txt")
    if os.path.isfile(requirements_txt):
        framework, runner = _detect_python_framework(requirements_txt)
        if framework != "unknown":
            return (framework, runner)

    # Check for setup.py as fallback
    setup_py = os.path.join(project_root, "setup.py")
    if os.path.isfile(setup_py):
        return ("pytest", "pytest")

    return ("unknown", "")


def _detect_node_framework(package_json_path: str) -> Tuple[str, str]:
    """
    Detect testing framework from package.json.

    Args:
        package_json_path: Path to package.json file.

    Returns:
        Tuple of (framework_name, runner_command).
    """
    try:
        with open(package_json_path, "r", encoding="utf-8") as f:
            pkg = json.load(f)

        # Merge dependencies and devDependencies
        deps = {
            **pkg.get("dependencies", {}),
            **pkg.get("devDependencies", {}),
        }

        # Check for specific frameworks (order matters)
        if "vitest" in deps:
            return ("vitest", "npx vitest run")

        if "jest" in deps:
            # Check for custom test script
            scripts = pkg.get("scripts", {})
            if "test" in scripts:
                return ("jest", "npm test")
            return ("jest", "npx jest")

        if "mocha" in deps:
            return ("mocha", "npx mocha")

        if "jasmine" in deps:
            return ("jasmine", "npx jasmine")

        # Check scripts for test command as hint
        scripts = pkg.get("scripts", {})
        test_script = scripts.get("test", "")
        if "vitest" in test_script:
            return ("vitest", "npm test")
        if "jest" in test_script:
            return ("jest", "npm test")
        if "mocha" in test_script:
            return ("mocha", "npm test")

        # Default to jest for Node.js projects
        if deps:
            return ("jest", "npm test")

    except (json.JSONDecodeError, OSError):
        pass

    return ("unknown", "")


def _detect_python_framework(requirements_path: str) -> Tuple[str, str]:
    """
    Detect testing framework from requirements.txt.

    Args:
        requirements_path: Path to requirements.txt file.

    Returns:
        Tuple of (framework_name, runner_command).
    """
    try:
        with open(requirements_path, "r", encoding="utf-8") as f:
            content = f.read().lower()

        if "pytest" in content:
            return ("pytest", "pytest")

        if "unittest" in content or "unittest2" in content:
            return ("unittest", "python -m unittest")

        if "nose" in content:
            return ("nose", "nosetests")

    except OSError:
        pass

    return ("unknown", "")


def get_test_file_extension(framework: str) -> str:
    """
    Get the appropriate test file extension for a framework.

    Args:
        framework: The framework name.

    Returns:
        File extension including the dot (e.g., ".test.js").
    """
    extensions = {
        "jest": ".test.js",
        "vitest": ".test.js",
        "mocha": ".test.js",
        "jasmine": ".spec.js",
        "pytest": "_test.py",
        "unittest": "_test.py",
        "nose": "_test.py",
        "cargo-test": "_test.rs",
    }
    return extensions.get(framework, ".test.js" if framework != "unknown" else "")


def get_test_dir(project_root: str, framework: str) -> str:
    """
    Get the standard test directory for a framework.

    Args:
        project_root: Path to the project root.
        framework: The framework name.

    Returns:
        Path to the test directory.
    """
    # Common test directory names
    test_dirs = ["tests", "test", "__tests__", "spec"]

    for test_dir in test_dirs:
        path = os.path.join(project_root, test_dir)
        if os.path.isdir(path):
            return path

    # Default based on framework
    if framework == "cargo-test":
        # Rust tests are colocated with source files
        return os.path.join(project_root, "src")
    elif framework in ("pytest", "unittest", "nose"):
        return os.path.join(project_root, "tests")
    else:
        return os.path.join(project_root, "__tests__")


def run_tests(project_root: str, framework: str, runner: str, test_file: str = None) -> Tuple[int, str, str]:
    """
    Run tests using the detected framework.

    Args:
        project_root: Path to the project root.
        framework: The framework name.
        runner: The test runner command.
        test_file: Optional specific test file to run.

    Returns:
        Tuple of (return_code, stdout, stderr).
    """
    import subprocess

    if framework == "unknown" or not runner:
        return (1, "", "No test framework detected")

    # Build command
    if test_file:
        cmd = f"{runner} {test_file}"
    else:
        cmd = runner

    try:
        # Use shell=True for Windows compatibility
        result = subprocess.run(
            cmd,
            cwd=project_root,
            shell=True,
            capture_output=True,
            text=True,
            timeout=300,  # 5 minute timeout
        )
        return (result.returncode, result.stdout, result.stderr)

    except subprocess.TimeoutExpired:
        return (124, "", "Test execution timed out (5 minutes)")
    except Exception as e:
        return (1, "", str(e))


# CLI interface for testing
if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: python test-framework.py <project_root>")
        sys.exit(1)

    project_root = sys.argv[1]
    framework, runner = detect_test_framework(project_root)

    print(f"Framework: {framework}")
    print(f"Runner: {runner}")
    print(f"Extension: {get_test_file_extension(framework)}")
    print(f"Test Dir: {get_test_dir(project_root, framework)}")