#!/usr/bin/env python3
"""
Lint and type check runner utility.

Encapsulates linting and type checking for various languages and frameworks,
returning structured results for gate decisions.

Supports --save-report to emit step reports for report-driven gates.
"""

import os
import subprocess
import sys
import time
from typing import Tuple, Optional, List, Dict
from dataclasses import dataclass

# Import step-report utility
try:
    import importlib.util
    _sr_path = os.path.join(os.path.dirname(__file__), "step-report.py")
    if os.path.isfile(_sr_path):
        _sr_spec = importlib.util.spec_from_file_location("step_report", _sr_path)
        _sr_module = importlib.util.module_from_spec(_sr_spec)
        _sr_spec.loader.exec_module(_sr_module)
        save_step_report = _sr_module.save_step_report
        HAS_STEP_REPORT = True
    else:
        HAS_STEP_REPORT = False
except Exception:
    HAS_STEP_REPORT = False


@dataclass
class LintResult:
    """Result of running lint/type checks."""
    success: bool
    return_code: int
    stdout: str
    stderr: str
    linter: str
    check_type: str  # "lint" or "type"
    errors: int = 0
    warnings: int = 0

    def summary(self) -> str:
        """Return a summary string of lint results."""
        if self.success:
            return f"✓ {self.linter} passed (0 errors)"
        else:
            return f"✗ {self.linter} failed ({self.errors} errors, {self.warnings} warnings)"

    def full_output(self) -> str:
        """Return full lint output."""
        parts = []
        if self.stdout:
            parts.append(self.stdout)
        if self.stderr:
            parts.append(f"STDERR:\n{self.stderr}")
        return "\n".join(parts)


def detect_project_type(project_root: str) -> Tuple[str, List[str]]:
    """
    Detect the project type and available linters.

    Returns: (primary_type, available_linters)
    - primary_type: "typescript", "javascript", "python", "mixed", "unknown"
    - available_linters: list of detected linters
    """
    available = []
    primary_type = "unknown"

    # Check for Node.js project
    package_json = os.path.join(project_root, "package.json")
    has_ts = False
    has_js = False

    if os.path.isfile(package_json):
        try:
            import json
            with open(package_json, "r", encoding="utf-8") as f:
                pkg = json.load(f)

            deps = {**pkg.get("dependencies", {}),
                    **pkg.get("devDependencies", {})}

            # Check for TypeScript
            if "typescript" in deps:
                has_ts = True
                available.append("tsc")
                primary_type = "typescript"

            # Check for ESLint
            if "eslint" in deps:
                available.append("eslint")

            # Check for other linters
            if "@typescript-eslint/parser" in deps:
                if "eslint" not in available:
                    available.append("eslint")

        except (json.JSONDecodeError, OSError):
            pass

        if not has_ts:
            has_js = True
            if primary_type == "unknown":
                primary_type = "javascript"

    # Check for oxlint (fast Rust-based linter) - preferred over ESLint
    if _is_command_available("oxlint"):
        available.insert(0, "oxlint")  # Add at front for priority

    # Check for Python project
    pyproject = os.path.join(project_root, "pyproject.toml")
    requirements = os.path.join(project_root, "requirements.txt")
    setup_py = os.path.join(project_root, "setup.py")
    has_py = False

    if os.path.isfile(pyproject) or os.path.isfile(requirements) or os.path.isfile(setup_py):
        has_py = True
        if primary_type in ("unknown", "javascript"):
            primary_type = "python" if primary_type == "unknown" else "mixed"

    # Check for Python linters in pyproject.toml
    if os.path.isfile(pyproject):
        try:
            with open(pyproject, "r", encoding="utf-8") as f:
                content = f.read()
                if "mypy" in content:
                    available.append("mypy")
                if "flake8" in content or "pyflakes" in content:
                    available.append("flake8")
                if "black" in content:
                    available.append("black")
                if "ruff" in content:
                    available.append("ruff")
        except OSError:
            pass

    # Check for installed Python linters via pip check
    for linter in ["mypy", "flake8", "black", "ruff"]:
        if linter not in available and _is_command_available(linter):
            # Only add if project has Python files
            if has_py:
                available.append(linter)

    # Add defaults if no linters detected
    if has_ts and "tsc" not in available:
        available.append("tsc")

    return primary_type, available


def _is_command_available(cmd: str) -> bool:
    """Check if a command is available in PATH."""
    try:
        result = subprocess.run(
            [cmd, "--version"],
            capture_output=True,
            timeout=5
        )
        return result.returncode == 0
    except (subprocess.TimeoutExpired, FileNotFoundError, OSError):
        return False


def run_type_check(
    project_root: str,
    check_type: Optional[str] = None,
    timeout: int = 120
) -> List[LintResult]:
    """
    Run type checking for the project.

    Args:
        project_root: Path to the project root directory.
        check_type: Optional specific type checker to use ("tsc", "mypy").
        timeout: Timeout in seconds.

    Returns:
        List of LintResult for each type checker run.
    """
    results = []
    project_type, available = detect_project_type(project_root)

    # TypeScript type check
    if check_type in (None, "tsc") and "tsc" in available:
        result = _run_tsc(project_root, timeout)
        results.append(result)

    # Python type check
    if check_type in (None, "mypy") and "mypy" in available:
        result = _run_mypy(project_root, timeout)
        results.append(result)

    return results


def run_lint(
    project_root: str,
    linter: Optional[str] = None,
    timeout: int = 120
) -> List[LintResult]:
    """
    Run linting for the project.

    Args:
        project_root: Path to the project root directory.
        linter: Optional specific linter to use ("oxlint", "eslint", "flake8", "black", "ruff").
        timeout: Timeout in seconds.

    Returns:
        List of LintResult for each linter run.
    """
    results = []
    project_type, available = detect_project_type(project_root)

    # oxlint (fast Rust-based linter) - prioritized over ESLint
    if linter in (None, "oxlint") and "oxlint" in available:
        result = _run_oxlint(project_root, timeout)
        results.append(result)
        # If oxlint runs successfully, skip ESLint for efficiency
        if result.success and linter is None:
            # Remove eslint from available to skip it
            available = [l for l in available if l != "eslint"]

    # ESLint
    if linter in (None, "eslint") and "eslint" in available:
        result = _run_eslint(project_root, timeout)
        results.append(result)

    # Flake8
    if linter in (None, "flake8") and "flake8" in available:
        result = _run_flake8(project_root, timeout)
        results.append(result)

    # Ruff (modern Python linter)
    if linter in (None, "ruff") and "ruff" in available:
        result = _run_ruff(project_root, timeout)
        results.append(result)

    # Black (format check)
    if linter in (None, "black") and "black" in available:
        result = _run_black_check(project_root, timeout)
        results.append(result)

    return results


def run_all_checks(
    project_root: str,
    timeout: int = 120
) -> Dict[str, List[LintResult]]:
    """
    Run all type checks and lints.

    Returns:
        Dict with "type" and "lint" keys containing list of results.
    """
    return {
        "type": run_type_check(project_root, timeout=timeout),
        "lint": run_lint(project_root, timeout=timeout),
    }


def _run_tsc(project_root: str, timeout: int) -> LintResult:
    """Run TypeScript type check (tsc --noEmit)."""
    cmd = ["npx", "tsc", "--noEmit"]

    try:
        result = subprocess.run(
            " ".join(cmd),
            cwd=project_root,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout
        )

        errors, warnings = _parse_tsc_output(result.stdout + result.stderr)

        return LintResult(
            success=result.returncode == 0,
            return_code=result.returncode,
            stdout=result.stdout,
            stderr=result.stderr,
            linter="tsc",
            check_type="type",
            errors=errors,
            warnings=warnings
        )

    except subprocess.TimeoutExpired:
        return LintResult(
            success=False,
            return_code=124,
            stdout="",
            stderr=f"Type check timed out after {timeout} seconds",
            linter="tsc",
            check_type="type"
        )
    except Exception as e:
        return LintResult(
            success=False,
            return_code=1,
            stdout="",
            stderr=str(e),
            linter="tsc",
            check_type="type"
        )


def _run_mypy(project_root: str, timeout: int) -> LintResult:
    """Run mypy type check."""
    cmd = ["mypy", "."]

    try:
        result = subprocess.run(
            " ".join(cmd),
            cwd=project_root,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout
        )

        errors, warnings = _parse_mypy_output(result.stdout + result.stderr)

        return LintResult(
            success=result.returncode == 0,
            return_code=result.returncode,
            stdout=result.stdout,
            stderr=result.stderr,
            linter="mypy",
            check_type="type",
            errors=errors,
            warnings=warnings
        )

    except subprocess.TimeoutExpired:
        return LintResult(
            success=False,
            return_code=124,
            stdout="",
            stderr=f"Type check timed out after {timeout} seconds",
            linter="mypy",
            check_type="type"
        )
    except Exception as e:
        return LintResult(
            success=False,
            return_code=1,
            stdout="",
            stderr=str(e),
            linter="mypy",
            check_type="type"
        )


def _run_eslint(project_root: str, timeout: int) -> LintResult:
    """Run ESLint."""
    cmd = ["npx", "eslint", ".", "--max-warnings=0"]

    try:
        result = subprocess.run(
            " ".join(cmd),
            cwd=project_root,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout
        )

        errors, warnings = _parse_eslint_output(result.stdout + result.stderr)

        return LintResult(
            success=result.returncode == 0,
            return_code=result.returncode,
            stdout=result.stdout,
            stderr=result.stderr,
            linter="eslint",
            check_type="lint",
            errors=errors,
            warnings=warnings
        )

    except subprocess.TimeoutExpired:
        return LintResult(
            success=False,
            return_code=124,
            stdout="",
            stderr=f"Lint timed out after {timeout} seconds",
            linter="eslint",
            check_type="lint"
        )
    except Exception as e:
        return LintResult(
            success=False,
            return_code=1,
            stdout="",
            stderr=str(e),
            linter="eslint",
            check_type="lint"
        )


def _run_oxlint(project_root: str, timeout: int) -> LintResult:
    """Run oxlint (fast Rust-based linter, ESLint-compatible)."""
    cmd = ["oxlint", "."]

    try:
        result = subprocess.run(
            " ".join(cmd),
            cwd=project_root,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout
        )

        errors, warnings = _parse_eslint_output(result.stdout + result.stderr)

        return LintResult(
            success=result.returncode == 0,
            return_code=result.returncode,
            stdout=result.stdout,
            stderr=result.stderr,
            linter="oxlint",
            check_type="lint",
            errors=errors,
            warnings=warnings
        )

    except subprocess.TimeoutExpired:
        return LintResult(
            success=False,
            return_code=124,
            stdout="",
            stderr=f"Lint timed out after {timeout} seconds",
            linter="oxlint",
            check_type="lint"
        )
    except Exception as e:
        return LintResult(
            success=False,
            return_code=1,
            stdout="",
            stderr=str(e),
            linter="oxlint",
            check_type="lint"
        )


def _run_flake8(project_root: str, timeout: int) -> LintResult:
    """Run flake8."""
    cmd = ["flake8", "."]

    try:
        result = subprocess.run(
            " ".join(cmd),
            cwd=project_root,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout
        )

        # flake8 outputs one line per error
        output = result.stdout + result.stderr
        errors = len([line for line in output.splitlines() if line.strip()])
        warnings = 0

        return LintResult(
            success=result.returncode == 0,
            return_code=result.returncode,
            stdout=result.stdout,
            stderr=result.stderr,
            linter="flake8",
            check_type="lint",
            errors=errors,
            warnings=warnings
        )

    except subprocess.TimeoutExpired:
        return LintResult(
            success=False,
            return_code=124,
            stdout="",
            stderr=f"Lint timed out after {timeout} seconds",
            linter="flake8",
            check_type="lint"
        )
    except Exception as e:
        return LintResult(
            success=False,
            return_code=1,
            stdout="",
            stderr=str(e),
            linter="flake8",
            check_type="lint"
        )


def _run_ruff(project_root: str, timeout: int) -> LintResult:
    """Run ruff linter."""
    cmd = ["ruff", "check", "."]

    try:
        result = subprocess.run(
            " ".join(cmd),
            cwd=project_root,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout
        )

        output = result.stdout + result.stderr
        errors = len([line for line in output.splitlines()
                     if line.strip() and not line.startswith("Found")])
        warnings = 0

        return LintResult(
            success=result.returncode == 0,
            return_code=result.returncode,
            stdout=result.stdout,
            stderr=result.stderr,
            linter="ruff",
            check_type="lint",
            errors=errors,
            warnings=warnings
        )

    except subprocess.TimeoutExpired:
        return LintResult(
            success=False,
            return_code=124,
            stdout="",
            stderr=f"Lint timed out after {timeout} seconds",
            linter="ruff",
            check_type="lint"
        )
    except Exception as e:
        return LintResult(
            success=False,
            return_code=1,
            stdout="",
            stderr=str(e),
            linter="ruff",
            check_type="lint"
        )


def _run_black_check(project_root: str, timeout: int) -> LintResult:
    """Run black format check."""
    cmd = ["black", "--check", "."]

    try:
        result = subprocess.run(
            " ".join(cmd),
            cwd=project_root,
            shell=True,
            capture_output=True,
            text=True,
            timeout=timeout
        )

        # Black shows files that would be reformatted
        output = result.stdout + result.stderr
        files_to_format = [line for line in output.splitlines()
                          if line.strip() and "would reformat" not in line.lower()]
        errors = len(files_to_format) if result.returncode != 0 else 0

        return LintResult(
            success=result.returncode == 0,
            return_code=result.returncode,
            stdout=result.stdout,
            stderr=result.stderr,
            linter="black",
            check_type="lint",
            errors=errors,
            warnings=0
        )

    except subprocess.TimeoutExpired:
        return LintResult(
            success=False,
            return_code=124,
            stdout="",
            stderr=f"Format check timed out after {timeout} seconds",
            linter="black",
            check_type="lint"
        )
    except Exception as e:
        return LintResult(
            success=False,
            return_code=1,
            stdout="",
            stderr=str(e),
            linter="black",
            check_type="lint"
        )


def _parse_tsc_output(output: str) -> Tuple[int, int]:
    """Parse TypeScript compiler output for error/warning counts."""
    import re

    # tsc format: error TS1234: Some error message
    errors = len(re.findall(r"\berror TS\d+:", output))
    warnings = len(re.findall(r"\bwarning TS\d+:", output))

    return errors, warnings


def _parse_mypy_output(output: str) -> Tuple[int, int]:
    """Parse mypy output for error/warning counts."""
    import re

    # mypy format: error: Some message
    errors = len(re.findall(r"\berror:", output))
    warnings = len(re.findall(r"\bwarning:", output))

    return errors, warnings


def _parse_eslint_output(output: str) -> Tuple[int, int]:
    """Parse ESLint output for error/warning counts."""
    import re

    # ESLint format: X problems (Y errors, Z warnings)
    match = re.search(r"(\d+)\s+errors?", output)
    errors = int(match.group(1)) if match else 0

    match = re.search(r"(\d+)\s+warnings?", output)
    warnings = int(match.group(1)) if match else 0

    return errors, warnings


# CLI interface
if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Run lint and type checks")
    parser.add_argument("project_root", help="Path to project root")
    parser.add_argument("--type-check", action="store_true",
                        help="Run type checking only")
    parser.add_argument("--lint", action="store_true",
                        help="Run linting only")
    parser.add_argument("--all", action="store_true",
                        help="Run all checks")
    parser.add_argument("--linter", help="Specific linter to use")
    parser.add_argument("--detect", action="store_true",
                        help="Detect available linters only")
    parser.add_argument("--save-report", action="store_true",
                        help="Save step report for report-driven gates")
    parser.add_argument("--change", help="Change name (required with --save-report)")
    parser.add_argument("--task-id", help="Task ID for report filename")

    args = parser.parse_args()

    if args.detect:
        project_type, available = detect_project_type(args.project_root)
        print(f"Project type: {project_type}")
        print(f"Available linters: {', '.join(available) if available else 'none'}")
        sys.exit(0)

    start_time = time.time()

    if args.all:
        results = run_all_checks(args.project_root)

        all_passed = True
        for check_type, check_results in results.items():
            for result in check_results:
                print(result.summary())
                if not result.success:
                    all_passed = False
                    print(result.full_output())

    elif args.type_check:
        results = run_type_check(args.project_root, check_type=args.linter)
        all_passed = all(r.success for r in results)
        for result in results:
            print(result.summary())
            if not result.success:
                print(result.full_output())

    elif args.lint:
        results = run_lint(args.project_root, linter=args.linter)
        all_passed = all(r.success for r in results)
        for result in results:
            print(result.summary())
            if not result.success:
                print(result.full_output())

    else:
        # Default: run all
        results = run_type_check(args.project_root)
        results.extend(run_lint(args.project_root))
        all_passed = all(r.success for r in results)
        for result in results:
            print(result.summary())
            if not result.success:
                print(result.full_output())

    elapsed_ms = int((time.time() - start_time) * 1000)

    if args.save_report and HAS_STEP_REPORT and args.change:
        total_errors = sum(r.errors for r in results if hasattr(r, 'errors'))
        total_warnings = sum(r.warnings for r in results if hasattr(r, 'warnings'))
        task_id = args.task_id or "0"
        save_step_report(
            change=args.change,
            task_id=task_id,
            step="lint",
            status="pass" if all_passed else "fail",
            details={
                "errors": total_errors,
                "warnings": total_warnings,
                "linters": [r.linter for r in results],
            },
            project_root=args.project_root,
            duration_ms=elapsed_ms,
        )

    sys.exit(0 if all_passed else 1)
