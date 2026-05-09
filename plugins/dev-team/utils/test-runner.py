#!/usr/bin/env python3
"""
Test runner utility.

Encapsulates test execution logic for various frameworks and returns
structured results for gate decisions.

Supports --save-report to emit step reports for report-driven gates.
"""

import importlib.util
import os
import subprocess
import sys
import time
from typing import Tuple, Optional, List
from dataclasses import dataclass

# Import step-report utility
try:
    import importlib.util as _ilu
    _sr_path = os.path.join(os.path.dirname(__file__), "step-report.py")
    if os.path.isfile(_sr_path):
        _sr_spec = _ilu.spec_from_file_location("step_report", _sr_path)
        _sr_module = _ilu.module_from_spec(_sr_spec)
        _sr_spec.loader.exec_module(_sr_module)
        save_step_report = _sr_module.save_step_report
        HAS_STEP_REPORT = True
    else:
        HAS_STEP_REPORT = False
except Exception:
    HAS_STEP_REPORT = False


def _import_test_framework():
    """Import test_framework module from the same directory."""
    module_path = os.path.join(os.path.dirname(__file__), "test-framework.py")
    spec = importlib.util.spec_from_file_location("test_framework", module_path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


# Lazy import
_test_framework_module = None


def _get_test_framework():
    global _test_framework_module
    if _test_framework_module is None:
        _test_framework_module = _import_test_framework()
    return _test_framework_module


@dataclass
class TestResult:
    """Result of running tests."""
    success: bool
    return_code: int
    stdout: str
    stderr: str
    framework: str
    test_file: str
    passed: int = 0
    failed: int = 0
    skipped: int = 0
    total: int = 0

    def summary(self) -> str:
        """Return a summary string of test results."""
        if self.success:
            return f"✓ All {self.total} tests passed"
        else:
            return f"✗ {self.failed}/{self.total} tests failed"

    def full_output(self) -> str:
        """Return full test output."""
        parts = []
        if self.stdout:
            parts.append(self.stdout)
        if self.stderr:
            parts.append(f"STDERR:\n{self.stderr}")
        return "\n".join(parts)


def run_tests(
    project_root: str,
    test_file: Optional[str] = None,
    test_pattern: Optional[str] = None,
    framework: Optional[str] = None,
    timeout: int = 300
) -> TestResult:
    """
    Run tests using the detected or specified framework.

    Args:
        project_root: Path to the project root directory.
        test_file: Optional specific test file to run.
        test_pattern: Optional test name pattern to match.
        framework: Optional framework override (jest, vitest, pytest).
        timeout: Timeout in seconds (default 5 minutes).

    Returns:
        TestResult with execution details.
    """
    # Detect framework if not specified
    if not framework:
        tf = _get_test_framework()
        framework, _ = tf.detect_test_framework(project_root)

    if framework == "unknown":
        return TestResult(
            success=False,
            return_code=1,
            stdout="",
            stderr="No test framework detected. Cannot run tests.",
            framework=framework,
            test_file=test_file or ""
        )

    # Build and execute command
    cmd, result = _execute_test_command(
        project_root, framework, test_file, test_pattern, timeout
    )

    # Parse results
    return _parse_test_result(result, framework, test_file or "")


def _execute_test_command(
    project_root: str,
    framework: str,
    test_file: Optional[str],
    test_pattern: Optional[str],
    timeout: int
) -> Tuple[List[str], subprocess.CompletedProcess]:
    """Execute the test command for the framework."""

    if framework in ("jest", "vitest", "mocha"):
        return _run_node_tests(project_root, framework, test_file, test_pattern, timeout)
    elif framework in ("pytest", "unittest", "nose"):
        return _run_python_tests(project_root, framework, test_file, test_pattern, timeout)
    else:
        raise ValueError(f"Unsupported framework: {framework}")


def _run_node_tests(
    project_root: str,
    framework: str,
    test_file: Optional[str],
    test_pattern: Optional[str],
    timeout: int
) -> Tuple[List[str], subprocess.CompletedProcess]:
    """Run Node.js tests (Jest, Vitest, Mocha)."""

    cmd_parts = []

    if framework == "vitest":
        cmd_parts = ["npx", "vitest", "run"]
        if test_file:
            cmd_parts.append(test_file)
        if test_pattern:
            cmd_parts.extend(["--testNamePattern", f'"{test_pattern}"'])
        cmd_parts.append("--passWithNoTests")

    elif framework == "jest":
        cmd_parts = ["npx", "jest"]
        if test_file:
            cmd_parts.append(test_file)
        if test_pattern:
            cmd_parts.extend(["--testNamePattern", test_pattern])
        cmd_parts.append("--passWithNoTests")

    elif framework == "mocha":
        cmd_parts = ["npx", "mocha"]
        if test_file:
            cmd_parts.append(test_file)
        else:
            # Default test directory
            test_dir = os.path.join(project_root, "test")
            if os.path.isdir(test_dir):
                cmd_parts.append(os.path.join(test_dir, "*.test.js"))

    try:
        result = subprocess.run(
            " ".join(cmd_parts),
            cwd=project_root,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout
        )
        return cmd_parts, result
    except subprocess.TimeoutExpired:
        # Create a fake result for timeout
        class TimeoutResult:
            returncode = 124
            stdout = ""
            stderr = f"Test execution timed out after {timeout} seconds"
        return cmd_parts, TimeoutResult()
    except Exception as e:
        class ErrorResult:
            returncode = 1
            stdout = ""
            stderr = str(e)
        return cmd_parts, ErrorResult()


def _run_python_tests(
    project_root: str,
    framework: str,
    test_file: Optional[str],
    test_pattern: Optional[str],
    timeout: int
) -> Tuple[List[str], subprocess.CompletedProcess]:
    """Run Python tests (pytest, unittest)."""

    cmd_parts = []

    if framework == "pytest":
        cmd_parts = ["pytest", "-v"]
        if test_file:
            cmd_parts.append(test_file)
        else:
            cmd_parts.append("tests/")
        if test_pattern:
            cmd_parts.extend(["-k", test_pattern])

    elif framework == "unittest":
        cmd_parts = ["python", "-m", "unittest", "-v"]
        if test_file:
            # Convert test file path to module path
            module_path = test_file.replace("/", ".").replace(".py", "")
            cmd_parts.append(module_path)

    elif framework == "nose":
        cmd_parts = ["nosetests", "-v"]
        if test_file:
            cmd_parts.append(test_file)

    try:
        result = subprocess.run(
            " ".join(cmd_parts),
            cwd=project_root,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout
        )
        return cmd_parts, result
    except subprocess.TimeoutExpired:
        class TimeoutResult:
            returncode = 124
            stdout = ""
            stderr = f"Test execution timed out after {timeout} seconds"
        return cmd_parts, TimeoutResult()
    except Exception as e:
        class ErrorResult:
            returncode = 1
            stdout = ""
            stderr = str(e)
        return cmd_parts, ErrorResult()


def _parse_test_result(
    result: subprocess.CompletedProcess,
    framework: str,
    test_file: str
) -> TestResult:
    """Parse test output to extract pass/fail counts."""

    success = result.returncode == 0
    stdout = result.stdout
    stderr = result.stderr

    # Parse counts based on framework
    passed, failed, skipped, total = 0, 0, 0, 0

    output = stdout + stderr

    if framework in ("jest", "vitest"):
        # Jest/Vitest format: "Tests: 3 passed, 1 failed, 4 total"
        import re
        match = re.search(r"Tests:\s*(\d+)\s*passed", output)
        if match:
            passed = int(match.group(1))
        match = re.search(r"(\d+)\s*failed", output)
        if match:
            failed = int(match.group(1))
        match = re.search(r"(\d+)\s*skipped", output)
        if match:
            skipped = int(match.group(1))
        total = passed + failed + skipped

    elif framework == "pytest":
        # pytest format: "3 passed, 1 failed"
        import re
        match = re.search(r"(\d+)\s*passed", output)
        if match:
            passed = int(match.group(1))
        match = re.search(r"(\d+)\s*failed", output)
        if match:
            failed = int(match.group(1))
        match = re.search(r"(\d+)\s*skipped", output)
        if match:
            skipped = int(match.group(1))
        match = re.search(r"(\d+)\s*warnings?", output)
        total = passed + failed + skipped

    return TestResult(
        success=success,
        return_code=result.returncode,
        stdout=stdout,
        stderr=stderr,
        framework=framework,
        test_file=test_file,
        passed=passed,
        failed=failed,
        skipped=skipped,
        total=total if total > 0 else (passed if passed > 0 else 1)
    )


def check_test_exists(
    project_root: str,
    source_file: str,
    framework: Optional[str] = None
) -> Optional[str]:
    """
    Check if a test file exists for a given source file.

    Args:
        project_root: Path to the project root.
        source_file: Path to the source file (relative to project root).
        framework: Optional framework override.

    Returns:
        Path to the test file if exists, None otherwise.
    """
    tf = _get_test_framework()
    if not framework:
        framework, _ = tf.detect_test_framework(project_root)

    # Map source file to test file
    test_dirs = ["tests", "test", "__tests__", "spec"]

    # Get base name without extension
    base_name = os.path.splitext(os.path.basename(source_file))[0]
    ext = tf.get_test_file_extension(framework)

    for test_dir in test_dirs:
        test_path = os.path.join(project_root, test_dir)

        # Try different naming conventions
        test_names = [
            f"{base_name}{ext}",
            f"{base_name}_test{ext}" if framework in ("pytest", "unittest") else None,
            f"test_{base_name}{ext}" if framework in ("pytest", "unittest") else None,
        ]

        for test_name in test_names:
            if test_name:
                test_file = os.path.join(test_path, test_name)
                if os.path.isfile(test_file):
                    return test_file

    return None


# CLI interface
if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Run tests for a project")
    parser.add_argument("project_root", help="Path to project root")
    parser.add_argument("--test-file", help="Specific test file to run")
    parser.add_argument("--test-pattern", help="Test name pattern to match")
    parser.add_argument("--framework", help="Test framework to use")
    parser.add_argument("--check-exists", help="Check if test exists for source file")
    parser.add_argument("--save-report", action="store_true",
                        help="Save step report for report-driven gates")
    parser.add_argument("--change", help="Change name (required with --save-report)")
    parser.add_argument("--task-id", help="Task ID for report filename")
    parser.add_argument("--step-name", default="scoped-test",
                        help="Step name for report (scoped-test or full-test)")

    args = parser.parse_args()

    if args.check_exists:
        test_file = check_test_exists(args.project_root, args.check_exists, args.framework)
        if test_file:
            print(f"Test file exists: {test_file}")
        else:
            print("No test file found")
        sys.exit(0)

    start_time = time.time()

    result = run_tests(
        args.project_root,
        test_file=args.test_file,
        test_pattern=args.test_pattern,
        framework=args.framework
    )

    elapsed_ms = int((time.time() - start_time) * 1000)

    if args.save_report and HAS_STEP_REPORT and args.change:
        task_id = args.task_id or "global"
        save_step_report(
            change=args.change,
            task_id=task_id,
            step=args.step_name,
            status="pass" if result.success else "fail",
            details={
                "passed": result.passed,
                "failed": result.failed,
                "skipped": result.skipped,
                "total": result.total,
                "framework": result.framework,
            },
            project_root=args.project_root,
            duration_ms=elapsed_ms,
        )

    print(result.summary())
    print("\n--- Test Output ---\n")
    print(result.full_output())

    sys.exit(result.return_code)