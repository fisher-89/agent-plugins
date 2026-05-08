#!/usr/bin/env python3
"""
Test scope identification utility.

Identifies the test scope for a task by analyzing the task description
and mapping affected source files to corresponding test files.
"""

import os
import re
import sys
import glob
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass, field


@dataclass
class TestScope:
    """Test scope for a task."""
    affected_files: List[str] = field(default_factory=list)
    """Source files affected by the task."""

    test_files: List[str] = field(default_factory=list)
    """Existing test files for the affected source files."""

    missing_test_files: List[str] = field(default_factory=list)
    """Test files that should exist but don't."""

    test_patterns: List[str] = field(default_factory=list)
    """Test name patterns extracted from task description."""

    needs_new_tests: bool = False
    """Whether new test files need to be generated."""

    keywords: List[str] = field(default_factory=list)
    """Keywords extracted from task description."""

    modules: List[str] = field(default_factory=list)
    """Module names identified from task description."""


def identify_test_scope(
    task: str,
    project_root: str,
    src_dir: Optional[str] = None,
    tests_dir: Optional[str] = None
) -> TestScope:
    """
    Identify test scope for a task.

    Args:
        task: Task description string.
        project_root: Path to the project root directory.
        src_dir: Optional source directory (defaults to src/).
        tests_dir: Optional tests directory (defaults to tests/ or __tests__/).

    Returns:
        TestScope with affected files, test files, and patterns.
    """
    project_root = os.path.abspath(project_root)

    # Determine source and test directories
    if not src_dir:
        src_dir = _find_src_dir(project_root)
    if not tests_dir:
        tests_dir = _find_tests_dir(project_root)

    scope = TestScope()

    # Extract keywords and modules from task
    scope.keywords = extract_keywords(task)
    scope.modules = extract_modules(task)
    scope.test_patterns = extract_test_patterns(task)

    # Strategy 1: Explicit file mentioned in task
    affected, tests, missing = _identify_by_explicit_file(task, src_dir, tests_dir)
    if affected:
        scope.affected_files.extend(affected)
        scope.test_files.extend(tests)
        scope.missing_test_files.extend(missing)
        scope.needs_new_tests = len(missing) > 0

    # Strategy 2: Feature module inference from keywords
    if not scope.affected_files:
        affected, tests, missing = _identify_by_keywords(scope.keywords, src_dir, tests_dir)
        scope.affected_files.extend(affected)
        scope.test_files.extend(tests)
        scope.missing_test_files.extend(missing)
        scope.needs_new_tests = len(missing) > 0

    # Strategy 3: Module names inference
    if not scope.affected_files:
        affected, tests, missing = _identify_by_modules(scope.modules, src_dir, tests_dir)
        scope.affected_files.extend(affected)
        scope.test_files.extend(tests)
        scope.missing_test_files.extend(missing)
        scope.needs_new_tests = len(missing) > 0

    # Strategy 4: Pattern-based inference (e.g., "routes", "handlers")
    if not scope.affected_files:
        affected, tests, missing = _identify_by_pattern(task, src_dir, tests_dir)
        scope.affected_files.extend(affected)
        scope.test_files.extend(tests)
        scope.missing_test_files.extend(missing)
        scope.needs_new_tests = len(missing) > 0

    return scope


def extract_keywords(task: str) -> List[str]:
    """
    Extract keywords from task description.

    Looks for domain concepts, feature names, and technical terms.
    """
    keywords = []

    # Common feature/module keywords
    feature_patterns = [
        r'\b(auth|authentication|login|logout|session)\b',
        r'\b(user|users?|account)\b',
        r'\b(api|endpoint|route|router)\b',
        r'\b(database|db|model|schema)\b',
        r'\b(cache|caching|redis)\b',
        r'\b(queue|worker|job)\b',
        r'\b(validation|validator|sanitize)\b',
        r'\b(middleware|interceptor)\b',
        r'\b(config|configuration|settings)\b',
        r'\b(utils?|helpers?|utilities)\b',
        r'\b(service|controller|handler)\b',
    ]

    for pattern in feature_patterns:
        matches = re.findall(pattern, task, re.IGNORECASE)
        keywords.extend([m.lower() for m in matches])

    # Remove duplicates while preserving order
    seen = set()
    unique = []
    for k in keywords:
        if k not in seen:
            seen.add(k)
            unique.append(k)

    return unique


def extract_modules(task: str) -> List[str]:
    """
    Extract module names from task description.

    Looks for explicit module references like "in auth.js" or "modify user.py".
    """
    modules = []

    # Pattern: "in <module>.<ext>" or "modify <module>.<ext>"
    file_patterns = [
        r'(?:in|modify|edit|update|create|implement)\s+([a-zA-Z_][a-zA-Z0-9_]*(?:/[a-zA-Z_][a-zA-Z0-9_]*)*\.[a-z]+)',
        r'([a-zA-Z_][a-zA-Z0-9_]*\.[a-z]+)',  # Simple filename
    ]

    for pattern in file_patterns:
        matches = re.findall(pattern, task, re.IGNORECASE)
        for m in matches:
            # Extract module name from path
            base = os.path.basename(m)
            module = os.path.splitext(base)[0]
            if module not in modules:
                modules.append(module)

    return modules


def extract_test_patterns(task: str) -> List[str]:
    """
    Extract test name patterns from task description.

    Used for running specific tests by name pattern.
    """
    patterns = []

    # Extract function/feature names
    # Pattern: "implement <name>", "add <name>", "create <name>"
    func_patterns = [
        r'(?:implement|add|create|build)\s+([a-zA-Z_][a-zA-Z0-9_]*)',
        r'(?:function|method|class)\s+([a-zA-Z_][a-zA-Z0-9_]*)',
        r'should\s+([a-z\s]+?)(?:\s+in|\s+for|\s*$)',
    ]

    for pattern in func_patterns:
        matches = re.findall(pattern, task, re.IGNORECASE)
        patterns.extend([m.strip().lower() for m in matches])

    # Remove duplicates
    return list(set(patterns))


def _find_src_dir(project_root: str) -> str:
    """Find the source directory in the project."""
    candidates = ["src", "lib", "app", "source", ""]
    for candidate in candidates:
        path = os.path.join(project_root, candidate)
        if os.path.isdir(path):
            return path
    return project_root


def _find_tests_dir(project_root: str) -> str:
    """Find the tests directory in the project."""
    candidates = ["tests", "test", "__tests__", "spec"]
    for candidate in candidates:
        path = os.path.join(project_root, candidate)
        if os.path.isdir(path):
            return path
    # Default to tests/
    return os.path.join(project_root, "tests")


def _identify_by_explicit_file(
    task: str,
    src_dir: str,
    tests_dir: str
) -> Tuple[List[str], List[str], List[str]]:
    """Identify scope by explicit file mention in task."""
    affected = []
    tests = []
    missing = []

    # Look for explicit file paths in task
    file_pattern = r'([a-zA-Z_][a-zA-Z0-9_]*(?:/[a-zA-Z_][a-zA-Z0-9_]*)*\.[a-z]+)'
    matches = re.findall(file_pattern, task)

    for match in matches:
        # Check if file exists in src_dir
        src_file = os.path.join(src_dir, match)
        if not os.path.isfile(src_file):
            # Maybe it's relative to project root
            src_file = match

        if os.path.isfile(src_file):
            affected.append(src_file)
            test_file = map_to_test_file(src_file, tests_dir)
            if os.path.isfile(test_file):
                tests.append(test_file)
            else:
                missing.append(test_file)

    return affected, tests, missing


def _identify_by_keywords(
    keywords: List[str],
    src_dir: str,
    tests_dir: str
) -> Tuple[List[str], List[str], List[str]]:
    """Identify scope by feature keywords."""
    affected = []
    tests = []
    missing = []

    # Map keywords to common file names
    keyword_files = {
        'auth': ['auth', 'authentication', 'login', 'session'],
        'user': ['user', 'users', 'account'],
        'api': ['api', 'routes', 'router', 'endpoints'],
        'database': ['db', 'database', 'models', 'schema'],
        'cache': ['cache', 'caching', 'redis'],
        'validation': ['validation', 'validator', 'validate'],
        'middleware': ['middleware', 'interceptor'],
        'config': ['config', 'configuration', 'settings'],
        'utils': ['utils', 'helpers', 'utilities'],
    }

    for keyword in keywords:
        # Get potential file names for keyword
        file_bases = keyword_files.get(keyword, [keyword])

        for base in file_bases:
            # Try common extensions
            for ext in ['.js', '.ts', '.py', '.jsx', '.tsx']:
                src_file = os.path.join(src_dir, f"{base}{ext}")
                if os.path.isfile(src_file):
                    affected.append(src_file)
                    test_file = map_to_test_file(src_file, tests_dir)
                    if os.path.isfile(test_file):
                        tests.append(test_file)
                    else:
                        missing.append(test_file)
                    break

    return affected, tests, missing


def _identify_by_modules(
    modules: List[str],
    src_dir: str,
    tests_dir: str
) -> Tuple[List[str], List[str], List[str]]:
    """Identify scope by module names."""
    affected = []
    tests = []
    missing = []

    for module in modules:
        # Try common extensions
        for ext in ['.js', '.ts', '.py', '.jsx', '.tsx']:
            src_file = os.path.join(src_dir, f"{module}{ext}")
            if os.path.isfile(src_file):
                affected.append(src_file)
                test_file = map_to_test_file(src_file, tests_dir)
                if os.path.isfile(test_file):
                    tests.append(test_file)
                else:
                    missing.append(test_file)
                break

    return affected, tests, missing


def _identify_by_pattern(
    task: str,
    src_dir: str,
    tests_dir: str
) -> Tuple[List[str], List[str], List[str]]:
    """Identify scope by pattern inference (e.g., routes, handlers)."""
    affected = []
    tests = []
    missing = []

    # Pattern-based inference
    patterns = {
        'routes': ['routes', 'router', 'api'],
        'handlers': ['handlers', 'controllers', 'views'],
        'models': ['models', 'entities', 'schemas'],
        'services': ['services', 'business'],
        'middleware': ['middleware', 'middlewares'],
    }

    task_lower = task.lower()
    for pattern, dirs in patterns.items():
        if pattern in task_lower:
            for dirname in dirs:
                dir_path = os.path.join(src_dir, dirname)
                if os.path.isdir(dir_path):
                    # Add all files in directory
                    for f in glob.glob(os.path.join(dir_path, '*')):
                        if os.path.isfile(f) and not f.endswith(('.md', '.txt', '.json')):
                            affected.append(f)
                            test_file = map_to_test_file(f, tests_dir)
                            if os.path.isfile(test_file):
                                tests.append(test_file)
                            else:
                                missing.append(test_file)

    return affected, tests, missing


def map_to_test_file(source_file: str, tests_dir: str = None) -> str:
    """
    Map a source file to its corresponding test file path.

    Tests are colocated with source files (tests_dir is ignored).

    Examples:
        src/routes/auth.js -> src/routes/auth.test.js
        src/auth.py -> src/test_auth.py
        lib/user.ts -> lib/user.test.ts
        src/auth.rs -> src/auth_tests.rs
    """
    basename = os.path.basename(source_file)
    name, ext = os.path.splitext(basename)
    dir_path = os.path.dirname(source_file)

    # Determine test file naming convention
    if ext == '.py':
        # Python: test_<name>.py
        test_name = f"test_{name}.py"
    elif ext == '.rs':
        # Rust: <name>_tests.rs
        test_name = f"{name}_tests.rs"
    else:
        # JavaScript/TypeScript: <name>.test.<ext>
        test_name = f"{name}.test{ext}"

    # Place test file in same directory as source file
    return os.path.join(dir_path, test_name)


def get_test_run_command(
    framework: str,
    test_files: List[str],
    test_pattern: Optional[str] = None
) -> str:
    """
    Generate test run command for the given scope.

    Args:
        framework: Test framework (jest, vitest, pytest).
        test_files: List of test files to run.
        test_pattern: Optional test name pattern.

    Returns:
        Command string to run the tests.
    """
    if not test_files:
        return ""

    if framework in ('jest', 'vitest'):
        cmd_parts = ['npx', framework, 'run' if framework == 'vitest' else framework]
        cmd_parts.extend(test_files)
        if test_pattern:
            cmd_parts.extend(['--testNamePattern', f'"{test_pattern}"'])
        cmd_parts.append('--passWithNoTests')
        return ' '.join(cmd_parts)

    elif framework == 'pytest':
        cmd_parts = ['pytest', '-v']
        cmd_parts.extend(test_files)
        if test_pattern:
            cmd_parts.extend(['-k', test_pattern])
        return ' '.join(cmd_parts)

    elif framework == 'cargo-test':
        cmd_parts = ['cargo', 'test']
        if test_pattern:
            cmd_parts.extend([test_pattern])
        return ' '.join(cmd_parts)

    return ""


# CLI interface
if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Identify test scope for a task")
    parser.add_argument("task", help="Task description")
    parser.add_argument("project_root", help="Path to project root")
    parser.add_argument("--src-dir", help="Source directory")
    parser.add_argument("--tests-dir", help="Tests directory")
    parser.add_argument("--json", action="store_true", help="Output as JSON")

    args = parser.parse_args()

    scope = identify_test_scope(
        args.task,
        args.project_root,
        args.src_dir,
        args.tests_dir
    )

    if args.json:
        import json
        print(json.dumps({
            "affected_files": scope.affected_files,
            "test_files": scope.test_files,
            "missing_test_files": scope.missing_test_files,
            "test_patterns": scope.test_patterns,
            "needs_new_tests": scope.needs_new_tests,
            "keywords": scope.keywords,
            "modules": scope.modules,
        }, indent=2))
    else:
        print("Test Scope:")
        print(f"  Keywords: {', '.join(scope.keywords) if scope.keywords else 'none'}")
        print(f"  Modules: {', '.join(scope.modules) if scope.modules else 'none'}")
        print(f"  Affected files: {len(scope.affected_files)}")
        for f in scope.affected_files:
            print(f"    - {f}")
        print(f"  Test files: {len(scope.test_files)}")
        for f in scope.test_files:
            print(f"    - {f}")
        print(f"  Missing tests: {len(scope.missing_test_files)}")
        for f in scope.missing_test_files:
            print(f"    - {f}")
        print(f"  Needs new tests: {scope.needs_new_tests}")
        print(f"  Test patterns: {', '.join(scope.test_patterns) if scope.test_patterns else 'none'}")
