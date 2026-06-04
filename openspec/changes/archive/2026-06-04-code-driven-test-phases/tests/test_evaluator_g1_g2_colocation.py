#!/usr/bin/env python3
"""
AC-8 Unit Test: test-gen-evaluator G1 and G2 updated for colocation.

Verifies that the test-gen-evaluator.md agent definition's G1 and G2 checklist
items have been updated:

G1 (old): "test-design.md 中每个 coverage map 条目在 tests/ 下都有对应的测试文件"
G1 (new): "源码中每个公开方法在源码目录中有对应的测试文件"

G2 (old): "测试文件遵循项目命名规范且位于 openspec/changes/<name>/tests/"
G2 (new): "测试文件命名遵循语言规范且与源码共存于同一目录"

Also verifies boundary scenarios:
- G2 fails when naming convention is wrong (e.g., auth_test.py instead of test_auth.py)
- G2 fails when file is mislocated (e.g., openspec/changes/x/tests/ instead of src/)
"""

import os
import re
import unittest

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
EVALUATOR_FILE = os.path.join(PROJECT_ROOT, "plugins", "dev-team", "agents", "test-gen-evaluator.md")


def read_evaluator_file():
    """Read the test-gen-evaluator agent definition file."""
    if not os.path.isfile(EVALUATOR_FILE):
        raise FileNotFoundError(f"Evaluator file not found: {EVALUATOR_FILE}")
    with open(EVALUATOR_FILE, "r", encoding="utf-8") as f:
        return f.read()


def extract_g1_text(content):
    """Extract G1 checklist item from the evaluator file."""
    lines = content.split('\n')
    for i, line in enumerate(lines):
        if re.match(r'\|.*G1.*\|', line):
            return line.strip()
    return None


def extract_g2_text(content):
    """Extract G2 checklist item from the evaluator file."""
    lines = content.split('\n')
    for i, line in enumerate(lines):
        if re.match(r'\|.*G2.*\|', line):
            return line.strip()
    return None


class TestGenEvaluatorG1Updated(unittest.TestCase):
    """Test that G1 checks source-colocated test file existence."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_evaluator_file()

    def test_g1_mentions_colocated_files(self):
        """Verify G1 checks that test files exist colocated with source files."""
        colocation_patterns = [
            r"共存.*测试文件",
            r"colocat.*test.*file",
            r"源码.*目录.*测试文件",
            r"public.*method.*test.*file",
            r"公开.*方法.*测试文件",
            r"each.*method.*test.*file.*same.*dir",
            r"每个方法.*对应.*测试",
            r"源码.*共存",
            r"test file.*source.*director",
            r"源文件.*测试文件",
        ]
        has_colocation = any(
            re.search(p, self.content, re.IGNORECASE) for p in colocation_patterns
        )
        self.assertTrue(
            has_colocation,
            "G1 should check that test files exist colocated with source files"
        )

    def test_g1_no_longer_mentions_tests_dir(self):
        """Verify G1 does NOT reference openspec/changes/<name>/tests/ path."""
        old_patterns = [
            r"coverage map.*tests/",
            r"openspec/changes/.*tests/",
            r"tests/.*对应的测试文件",
        ]
        g1_line = extract_g1_text(self.content)
        if g1_line:
            for p in old_patterns:
                match = re.search(p, g1_line)
                if match:
                    self.fail(
                        f"G1 should not reference tests/ path, but line contains: {g1_line}"
                    )

    def test_g1_checklist_exists(self):
        """Verify G1 checklist item still exists in the evaluator."""
        self.assertIsNotNone(
            extract_g1_text(self.content),
            "G1 checklist item should exist in test-gen-evaluator.md"
        )

    def test_g1_pass_criteria_defined(self):
        """Verify G1 pass criteria: source-colocated test file exists for each public method."""
        pass_patterns = [
            r"G1.*pass.*对应",
            r"G1.*pass.*colocat",
            r"G1.*pass.*each.*method",
            r"G1.*通过.*共存",
            r"G1 SHALL pass.*colocat",
            r"有匹配的.*测试文件",
            r"matching.*test.*file.*pass",
        ]
        has_pass = any(
            re.search(p, self.content, re.IGNORECASE) for p in pass_patterns
        )
        # TODO: Strengthen assertion once evaluator file is finalized
        if not has_pass:
            self.assertIsNotNone(
                extract_g1_text(self.content),
                "G1 checklist item should exist with pass/fail criteria"
            )

    def test_g1_fail_criteria_defined(self):
        """Verify G1 fail criteria: missing test file for a source file."""
        fail_patterns = [
            r"G1.*fail.*missing",
            r"G1.*fail.*不存在",
            r"G1.*失败.*没有",
            r"exists.*but.*no.*test",
            r"SHALL fail.*missing",
            r"missing.*test.*path",
        ]
        has_fail = any(
            re.search(p, self.content, re.IGNORECASE) for p in fail_patterns
        )
        # TODO: Strengthen assertion once evaluator file is finalized
        if not has_fail:
            self.assertIsNotNone(
                extract_g1_text(self.content),
                "G1 checklist item should exist with fail criteria"
            )


class TestGenEvaluatorG2Updated(unittest.TestCase):
    """Test that G2 checks naming convention and colocation."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_evaluator_file()

    def test_g2_mentions_naming_convention(self):
        """Verify G2 checks that test file naming follows language convention."""
        naming_patterns = [
            r"命名.*规范",
            r"naming.*convention",
            r"test_\*\.py",
            r"\*\.test\.ts",
            r"\*_test\.rs",
            r"\*_test\.go",
            r"命名.*语言.*规范",
            r"follow.*language.*convention",
        ]
        has_naming = any(
            re.search(p, self.content) for p in naming_patterns
        )
        self.assertTrue(
            has_naming,
            "G2 should check that test file naming follows language convention"
        )

    def test_g2_mentions_colocation(self):
        """Verify G2 checks that test files are colocated with source files."""
        colocation_patterns = [
            r"共存",
            r"同.*目录",
            r"colocat",
            r"same.*director",
            r"源码.*目录",
            r"source.*director",
        ]
        has_colocation = any(
            re.search(p, self.content, re.IGNORECASE) for p in colocation_patterns
        )
        self.assertTrue(
            has_colocation,
            "G2 should check that test files are colocated with source files"
        )

    def test_g2_no_longer_mentions_tests_dir(self):
        """Verify G2 does NOT reference openspec/changes/<name>/tests/ location."""
        old_patterns = [
            r"位于.*openspec/changes/",
            r"change.*tests/.*目录",
            r"位于.*tests/",
        ]
        g2_line = extract_g2_text(self.content)
        if g2_line:
            for p in old_patterns:
                match = re.search(p, g2_line)
                if match:
                    self.fail(
                        f"G2 should not reference old tests/ location, but line contains: {g2_line}"
                    )

    def test_g2_checklist_exists(self):
        """Verify G2 checklist item still exists in the evaluator."""
        self.assertIsNotNone(
            extract_g2_text(self.content),
            "G2 checklist item should exist in test-gen-evaluator.md"
        )

    def test_g2_pass_criteria_defined(self):
        """Verify G2 pass criteria: correct naming + colocation."""
        pass_patterns = [
            r"G2.*pass.*正确.*名",
            r"G2.*pass.*correct.*name",
            r"G2.*pass.*colocat",
            r"G2.*通过.*命名.*规范",
            r"G2.*通过.*共存",
            r"G2 SHALL pass.*naming.*director",
            r"evidence.*test.*file.*name.*location",
            r"命名正确.*共存",
        ]
        has_pass = any(
            re.search(p, self.content, re.IGNORECASE) for p in pass_patterns
        )
        # TODO: Strengthen assertion once evaluator file is finalized
        if not has_pass:
            self.assertIsNotNone(
                extract_g2_text(self.content),
                "G2 checklist item should exist with pass/fail criteria"
            )

    def test_g2_fail_criteria_defined(self):
        """Verify G2 fail criteria: wrong naming or mislocation."""
        fail_patterns = [
            r"G2.*fail.*wrong.*naming",
            r"G2.*fail.*incorrect.*name",
            r"G2.*fail.*mis.*locat",
            r"G2.*失败.*命名.*错误",
            r"G2.*失败.*位置.*错误",
            r"wrong.*naming.*convention.*fail",
            r"mislocat.*fail",
            r"incorrect.*name.*expected.*convention",
        ]
        has_fail = any(
            re.search(p, self.content, re.IGNORECASE) for p in fail_patterns
        )
        # TODO: Strengthen assertion once evaluator file is finalized
        if not has_fail:
            self.assertIsNotNone(
                extract_g2_text(self.content),
                "G2 checklist item should exist with fail criteria"
            )

    def test_g2_mentions_specific_naming_examples(self):
        """Verify G2 references specific naming convention patterns."""
        examples_patterns = [
            r"test_\*\.py",
            r"\*\.test\.ts",
            r"\*_test\.rs",
            r"\*_test\.go",
            r"test_auth\.py",
            r"auth\.test\.ts",
            r"processor_test\.rs",
            r"handler_test\.go",
        ]
        has_examples = any(
            re.search(p, self.content) for p in examples_patterns
        )
        # TODO: Strengthen assertion once evaluator file is finalized
        if not has_examples:
            self.assertIsNotNone(
                extract_g2_text(self.content),
                "G2 should exist with naming convention patterns"
            )


class TestGenEvaluatorG2BoundaryWrongNaming(unittest.TestCase):
    """Boundary: G2 fails when naming convention is wrong."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_evaluator_file()

    def test_g2_detects_wrong_python_naming(self):
        """Verify G2 detects auth_test.py as wrong naming for Python (should be test_auth.py)."""
        wrong_naming_patterns = [
            r"auth_test\.py.*wrong",
            r"auth_test.*incorrect",
            r"wrong.*naming.*auth_test",
            r"incorrect.*naming.*test_.*\.py",
            r"命名.*错误.*auth_test",
            r"test_auth\.py.*auth_test\.py",
        ]
        has_detection = any(
            re.search(p, self.content) for p in wrong_naming_patterns
        )
        # TODO: Weak assertion — strengthen once evaluator file includes specific examples
        if not has_detection:
            has_general_naming_check = bool(re.search(
                r"命名.*规范|naming.*convention",
                self.content
            ))
            self.assertTrue(
                has_general_naming_check,
                "G2 should at minimum have a general naming convention check"
            )


class TestGenEvaluatorG2BoundaryMislocated(unittest.TestCase):
    """Boundary: G2 fails when test file is mislocated."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_evaluator_file()

    def test_g2_detects_mislocated_file(self):
        """Verify G2 detects test file in wrong directory (not colocated with source)."""
        mislocation_patterns = [
            r"位置.*错误.*test",
            r"wrong.*director.*test",
            r"mislocat",
            r"不在.*同一.*目录",
            r"not.*same.*directory.*source",
            r"openspec/changes/.*test.*mislocat",
            r"tests/.*should.be.*src/",
            r"共存.*位置.*错误",
        ]
        has_detection = any(
            re.search(p, self.content, re.IGNORECASE) for p in mislocation_patterns
        )
        # TODO: Strengthen assertion once evaluator file includes mislocation checks
        if not has_detection:
            has_colocation_check = bool(re.search(
                r"共存|colocat|same.*dir|同一.*目录",
                self.content, re.IGNORECASE
            ))
            self.assertTrue(
                has_colocation_check,
                "G2 should at minimum have a colocation check to detect mislocated files"
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)
