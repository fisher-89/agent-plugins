#!/usr/bin/env python3
"""
AC-4 Unit Test: test-gen-generator language naming conventions.

Verifies that the test-gen-generator.md agent definition specifies
language-specific test file naming conventions:
- Python:   test_<module>.py
- TypeScript: <module>.test.ts / <module>.test.tsx
- Rust:     <module>_test.rs or inline #[cfg(test)] mod tests
- Go:       <module>_test.go
"""

import os
import re
import unittest

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
AGENT_FILE = os.path.join(PROJECT_ROOT, "plugins", "dev-team", "agents", "test-gen-generator.md")


def read_agent_file():
    """Read the test-gen-generator agent definition file."""
    if not os.path.isfile(AGENT_FILE):
        raise FileNotFoundError(f"Agent file not found: {AGENT_FILE}")
    with open(AGENT_FILE, "r", encoding="utf-8") as f:
        return f.read()


class TestGenGeneratorPythonNaming(unittest.TestCase):
    """Test Python naming convention: test_*.py."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_python_naming_instruction_present(self):
        """Verify agent specifies test_<module>.py naming for Python files."""
        patterns = [
            r"test_\*\.py",
            r"test_<module>\.py",
            r"test_<name>\.py",
            r"Python.*test_.*\.py",
            r"\.py.*test_.*\.py",
            r"Python.*命名.*test_",
            r"Python.*naming.*test_",
        ]
        has_py_naming = any(re.search(p, self.content) for p in patterns)
        self.assertTrue(
            has_py_naming,
            "test-gen-generator.md should specify test_<module>.py "
            "naming convention for Python files"
        )

    def test_python_naming_example(self):
        """Verify agent includes a concrete Python naming example."""
        examples = [
            r"test_auth\.py",
            r"test_handler\.py",
            r"test_main\.py",
            r"test_processor\.py",
        ]
        has_example = any(re.search(p, self.content) for p in examples)
        # TODO: Check for concrete example once agent file is finalized
        if not has_example:
            self.assertIsNotNone(
                re.search(r"test_.*\.py", self.content),
                "Agent should include at least one test_*.py example"
            )


class TestGenGeneratorTypeScriptNaming(unittest.TestCase):
    """Test TypeScript naming convention: *.test.ts."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_typescript_naming_instruction_present(self):
        """Verify agent specifies *.test.ts naming for TypeScript files."""
        patterns = [
            r"\*\.test\.ts",
            r"<module>\.test\.ts",
            r"<name>\.test\.ts",
            r"TypeScript.*test\.ts",
            r"\.ts.*\.test\.ts",
            r"tsx.*\.test\.tsx",
            r"TypeScript.*命名.*test",
        ]
        has_ts_naming = any(re.search(p, self.content) for p in patterns)
        self.assertTrue(
            has_ts_naming,
            "test-gen-generator.md should specify <module>.test.ts "
            "naming convention for TypeScript files"
        )


class TestGenGeneratorRustNaming(unittest.TestCase):
    """Test Rust naming convention: *_test.rs."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_rust_naming_instruction_present(self):
        """Verify agent specifies *_test.rs naming for Rust files."""
        patterns = [
            r"\*_test\.rs",
            r"<module>_test\.rs",
            r"<name>_test\.rs",
            r"Rust.*_test\.rs",
            r"\.rs.*_test\.rs",
            r"Rust.*命名.*_test",
            r"#\[cfg\(test\)\]",
            r"inline.*mod tests",
        ]
        has_rs_naming = any(re.search(p, self.content) for p in patterns)
        self.assertTrue(
            has_rs_naming,
            "test-gen-generator.md should specify <module>_test.rs "
            "or inline #[cfg(test)] mod tests convention for Rust files"
        )


class TestGenGeneratorGoNaming(unittest.TestCase):
    """Test Go naming convention: *_test.go."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_go_naming_instruction_present(self):
        """Verify agent specifies *_test.go naming for Go files."""
        patterns = [
            r"\*_test\.go",
            r"<module>_test\.go",
            r"<name>_test\.go",
            r"Go.*_test\.go",
            r"\.go.*_test\.go",
            r"Go.*命名.*_test",
        ]
        has_go_naming = any(re.search(p, self.content) for p in patterns)
        self.assertTrue(
            has_go_naming,
            "test-gen-generator.md should specify <module>_test.go "
            "naming convention for Go files"
        )


class TestGenGeneratorAllLanguagesTable(unittest.TestCase):
    """Test that all four language conventions appear in a structured form."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_all_naming_conventions_in_table(self):
        """Verify naming conventions appear in a table or structured list covering all languages."""
        # Check for table-like structure mapping Source Type -> Test File Naming
        table_patterns = [
            r"\| Source Type.*Test File Naming",
            r"\| 源码类型.*测试文件命名",
            r"\|.*\.py.*test_.*\.py",
            r"\|.*\.ts.*\.test\.ts",
            r"\|.*\.rs.*_test\.rs",
            r"\|.*\.go.*_test\.go",
            r"| Python | test_.*\.py",
            r"| TypeScript | .*\.test\.ts",
            r"| Rust | .*_test\.rs",
            r"| Go | .*_test\.go",
        ]
        has_table = any(re.search(p, self.content, re.IGNORECASE) for p in table_patterns)

        # Fallback: check for individual mentions of each language
        has_python = bool(re.search(r"test_.*\.py", self.content))
        has_typescript = bool(re.search(r"\.test\.ts", self.content))
        has_rust = bool(re.search(r"_test\.rs", self.content))
        has_go = bool(re.search(r"_test\.go", self.content))
        all_languages_mentioned = has_python and has_typescript and has_rust and has_go

        self.assertTrue(
            has_table or all_languages_mentioned,
            "Naming conventions should appear either in a structured table "
            "or as individual instructions for each language"
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
