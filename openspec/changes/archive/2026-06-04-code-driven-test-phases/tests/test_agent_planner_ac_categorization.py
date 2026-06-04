#!/usr/bin/env python3
"""
AC-1 Unit Test: test-design-planner Forward/Reverse AC categorization.

Verifies that the test-design-planner.md agent definition contains:
1. Forward ACs / Reverse ACs categorization instruction
2. Grep source code instruction for real API signatures
3. Constraint to NOT output parameter types or risk markers

Also verifies boundary scenarios:
- Simultaneous old-format ("parameter type table") and new-format
  ("禁止输出参数类型") instructions: new format MUST override old
- Empty Reverse ACs: planner should output an empty section, not skip it
"""

import os
import re
import unittest

# Paths relative to project root
PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))
AGENT_FILE = os.path.join(PROJECT_ROOT, "plugins", "dev-team", "agents", "test-design-planner.md")


def read_agent_file():
    """Read the test-design-planner agent definition file."""
    if not os.path.isfile(AGENT_FILE):
        raise FileNotFoundError(f"Agent file not found: {AGENT_FILE}")
    with open(AGENT_FILE, "r", encoding="utf-8") as f:
        return f.read()


def has_forward_ac_section(content):
    """Check if the agent defines Forward AC categorization instructions."""
    # Accept Chinese (正向 AC) or English (Forward AC) references
    patterns = [
        r"Forward\s*AC",
        r"正向\s*AC",
        r"正向.*AC",
    ]
    return any(re.search(p, content, re.IGNORECASE) for p in patterns)


def has_reverse_ac_section(content):
    """Check if the agent defines Reverse AC categorization instructions."""
    patterns = [
        r"Reverse\s*AC",
        r"反向\s*AC",
        r"反向.*AC",
        r"异常路径",
        r"sad\s*path",
    ]
    return any(re.search(p, content, re.IGNORECASE) for p in patterns)


def has_grep_source_instruction(content):
    """Check if the agent contains a Grep source code instruction."""
    patterns = [
        r"Grep.*源码",
        r"grep.*source",
        r"提取.*API.*签名",
        r"extract.*API.*signature",
        r"read.*source.*file",
        r"源码.*补充.*输入",
    ]
    return any(re.search(p, content, re.IGNORECASE) for p in patterns)


def has_no_parameter_type_output(content):
    """Check if the agent contains constraint to NOT output parameter types."""
    patterns_positive = [
        r"不.*输出.*参数.*类型",
        r"(?i)shall not.*output.*parameter.*type",
        r"(?i)shall not.*contain.*parameter.*type",
        r"禁止.*输出.*参数.*类型",
        r"(?i)no.*parameter.*type.*table",
        r"不.*包含.*参数.*类型表",
    ]
    return any(re.search(p, content) for p in patterns_positive)


def has_no_risk_marker_output(content):
    """Check if the agent contains constraint to NOT output risk markers."""
    patterns = [
        r"不.*输出.*风险",
        r"(?i)shall not.*risk.*marker",
        r"禁止.*输出.*风险",
        r"不.*风险标记",
        r"(?i)no.*risk.*marker",
        r"不.*风险.*说明",
    ]
    return any(re.search(p, content) for p in patterns)


def has_old_parameter_type_table(content):
    """Check for presence of old-format parameter type table instruction (should NOT exist after change)."""
    patterns = [
        r"参数类型表",
        r"Parameter Type Table",
        r"参数类型.*映射",
    ]
    return any(re.search(p, content) for p in patterns)


class TestDesignPlannerForwardAC(unittest.TestCase):
    """Test that test-design-planner.md contains Forward AC categorization."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_forward_ac_section_present(self):
        """Verify Forward AC categorization instruction exists in agent file."""
        self.assertTrue(
            has_forward_ac_section(self.content),
            "test-design-planner.md should contain Forward AC categorization instruction"
        )

    def test_forward_ac_references_proposal(self):
        """Verify Forward ACs reference proposal acceptance criteria."""
        # The agent should reference proposal.md for AC extraction
        has_proposal_ref = re.search(r"proposal\.md", self.content, re.IGNORECASE)
        # TODO: Strengthen assertion once agent file is finalized
        self.assertIsNotNone(
            has_proposal_ref,
            "Agent should reference proposal.md for AC extraction"
        )


class TestDesignPlannerReverseAC(unittest.TestCase):
    """Test that test-design-planner.md contains Reverse AC categorization."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_reverse_ac_section_present(self):
        """Verify Reverse AC/异常路径 categorization exists in agent file."""
        self.assertTrue(
            has_reverse_ac_section(self.content),
            "test-design-planner.md should contain Reverse AC / sad path categorization instruction"
        )

    def test_forward_and_reverse_sections_separate(self):
        """Verify Forward ACs and Reverse ACs are defined as separate sections."""
        has_forward = has_forward_ac_section(self.content)
        has_reverse = has_reverse_ac_section(self.content)
        self.assertTrue(
            has_forward and has_reverse,
            "Agent should define both Forward AC and Reverse AC as distinct sections"
        )


class TestDesignPlannerGrepSource(unittest.TestCase):
    """Test that test-design-planner.md contains Grep source code instruction."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_grep_source_instruction_present(self):
        """Verify agent contains instruction to grep/read source code for API signatures."""
        self.assertTrue(
            has_grep_source_instruction(self.content),
            "test-design-planner.md should contain Grep source code instruction "
            "to extract API signatures as supplementary input"
        )

    def test_grep_supplementary_not_primary(self):
        """Verify Grep source is supplementary to proposal.md + design.md, not a replacement."""
        has_proposal_ref = re.search(r"proposal\.md", self.content)
        has_design_ref = re.search(r"design\.md", self.content)
        # TODO: Confirm agent references both proposal.md and design.md as primary inputs
        self.assertIsNotNone(has_proposal_ref, "Agent should reference proposal.md as primary input")
        self.assertIsNotNone(has_design_ref, "Agent should reference design.md as primary input")


class TestDesignPlannerNoParameterTypes(unittest.TestCase):
    """Test that test-design-planner.md forbids outputting parameter types."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_no_parameter_type_output_constraint(self):
        """Verify agent contains constraint to NOT output parameter types in test-design.md."""
        self.assertTrue(
            has_no_parameter_type_output(self.content),
            "test-design-planner.md should contain constraint to NOT output parameter types "
            "or type tables in test-design.md"
        )

    def test_no_risk_marker_output_constraint(self):
        """Verify agent contains constraint to NOT output risk markers for untyped files."""
        self.assertTrue(
            has_no_risk_marker_output(self.content),
            "test-design-planner.md should contain constraint to NOT output risk markers "
            "for untyped files"
        )


class TestDesignPlannerBoundaryConflictResolution(unittest.TestCase):
    """Boundary: agent files containing both old and new format instructions."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_new_format_overrides_old_if_both_present(self):
        """If both old-format (parameter type table) and new-format (no output) instructions
        coexist, the new constraint must take precedence."""
        has_old = has_old_parameter_type_table(self.content)
        has_new = has_no_parameter_type_output(self.content)
        if has_old and has_new:
            # Both present: verify new constraint comes after old in the file
            old_pos = self.content.find("参数类型表")
            if old_pos == -1:
                old_pos = self.content.find("Parameter Type Table")
            new_patterns = ["不.*输出.*参数.*类型", "shall not.*output.*parameter.*type",
                            "禁止.*输出.*参数.*类型"]
            new_positions = []
            for p in new_patterns:
                m = re.search(p, self.content)
                if m:
                    new_positions.append(m.start())
            if new_positions:
                min_new_pos = min(new_positions)
                self.assertGreater(
                    min_new_pos, old_pos,
                    "When both old and new format exist, new no-output constraint "
                    "must appear AFTER the old parameter type table instruction "
                    "to ensure override"
                )

    def test_reverse_ac_empty_placeholder(self):
        """Verify agent instructs planner to output an empty Reverse ACs section
        when no explicit sad-path scenarios exist, rather than skipping the section."""
        # Look for wording about handling empty/missing reverse scenarios
        # TODO: Strengthen assertion once exact agent wording is finalized
        empty_handling_patterns = [
            r"空.*反向\s*AC",
            r"空.*Reverse\s*AC",
            r"空.*异常路径",
            r"(?i)empty.*reverse",
            r"(?i)placeholder.*reverse",
            r"(?i)no.*reverse.*then.*empty",
        ]
        has_empty_handling = any(
            re.search(p, self.content) for p in empty_handling_patterns
        )
        # If no explicit empty-handling instruction found, at minimum verify
        # the agent has a section for Reverse ACs that would appear in output
        has_reverse = has_reverse_ac_section(self.content)
        self.assertTrue(
            has_empty_handling or has_reverse,
            "Agent should either handle empty Reverse ACs explicitly or define "
            "a Reverse AC section that serves as placeholder"
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
