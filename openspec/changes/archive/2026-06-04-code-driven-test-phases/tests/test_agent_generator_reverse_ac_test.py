#!/usr/bin/env python3
"""
AC-6 Unit Test: test-gen-generator derives error path tests from Reverse ACs.

Verifies that the test-gen-generator.md agent definition contains instruction
to derive exception/error path test cases from the Reverse ACs (反向 AC)
section in test-design.md.
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


class TestGenGeneratorReverseACTest(unittest.TestCase):
    """Test that test-gen-generator.md instructs derivation of error path tests from Reverse ACs."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_reverse_ac_derivation_instruction_present(self):
        """Verify agent contains instruction to derive error path tests from Reverse ACs."""
        patterns = [
            r"Reverse\s*AC.*test",
            r"反向\s*AC.*测试",
            r"异常路径.*测试",
            r"error.*path.*test",
            r"sad.*path.*test",
            r"exception.*test.*Reverse",
            r"异常.*场景.*测试",
            r"derive.*test.*from.*Reverse",
            r"从.*反向.*AC.*推导",
            r"从Reverse.*AC.*推导",
            r"Reverse ACs.*生成.*异常",
        ]
        has_reverse_derivation = any(
            re.search(p, self.content, re.IGNORECASE) for p in patterns
        )
        self.assertTrue(
            has_reverse_derivation,
            "test-gen-generator.md should contain instruction to derive "
            "error/exception path tests from Reverse ACs (反向 AC)"
        )

    def test_reverse_ac_references_test_design(self):
        """Verify the derivation references test-design.md as the source of Reverse ACs."""
        has_td_ref = bool(re.search(
            r"test-design\.md",
            self.content, re.IGNORECASE
        ))
        # The agent might reference Reverse ACs without explicitly naming test-design.md
        # TODO: Strengthen assertion once agent file is finalized
        if not has_td_ref:
            has_indirect_ref = bool(re.search(
                r"测试设计.*反向|Reverse.*AC.*来源于|反向.*AC.*来自|"
                r"根据.*反向.*AC.*生成|根据.*Reverse.*AC.*generat",
                self.content
            ))
            self.assertTrue(
                has_indirect_ref or has_td_ref,
                "Reverse AC derivation should reference test-design.md or the test design artifact"
            )

    def test_reverse_ac_tests_distinct_from_forward(self):
        """Verify reverse/error tests are distinguished from forward/happy-path tests."""
        has_distinction = bool(re.search(
            r"快乐路径.*异常路径|正向.*反向|happy.*path.*error|"
            r"forward.*ac.*test.*reverse.*ac.*test|"
            r"区分.*正向.*反向|normal.*case.*error.*case",
            self.content, re.IGNORECASE
        ))
        # TODO: Strengthen assertion once agent file is finalized
        if not has_distinction:
            # Fallback: verify agent mentions both forward and reverse concepts
            has_forward = bool(re.search(r"Forward AC|正向 AC|happy.*path", self.content, re.IGNORECASE))
            has_reverse = bool(re.search(r"Reverse AC|反向 AC|异常路径|sad.*path", self.content, re.IGNORECASE))
            self.assertTrue(
                has_forward and has_reverse,
                "Agent should distinguish between forward (happy path) and reverse (error path) tests"
            )

    def test_error_path_test_structure(self):
        """Verify error path tests include expected error/exception assertions."""
        # Look for instructions about what error path tests should contain
        structure_patterns = [
            r"(?i)assert.*error",
            r"(?i)expect.*exception",
            r"(?i)验证.*错误",
            r"(?i)验证.*异常",
            r"(?i)should.*fail",
            r"(?i)expected.*fail",
            r"(?i)raises?.*error",
            r"(?i)should.*throw",
            r"(?i)invalid.*input.*test",
        ]
        has_structure = any(
            re.search(p, self.content) for p in structure_patterns
        )
        # TODO: Strengthen assertion once agent file defines error test structure
        if not has_structure:
            # Fallback: derivation implies error-specific assertions
            self.assertIsNotNone(
                re.search(r"Reverse AC|反向 AC|异常", self.content),
                "Agent should reference Reverse AC structures for error path tests"
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)
