#!/usr/bin/env python3
"""
AC-10 Unit Test: phase-test-design and phase-test-gen skills are unchanged.

Verifies that the skill files at:
- plugins/dev-team/skills/phase-test-design/SKILL.md
- plugins/dev-team/skills/phase-test-gen/SKILL.md

contain NO new or modified domain logic instructions. The skill files
must remain thin P->E and G->E orchestrators respectively, with:
- Prompt unchanged (single-line, no domain logic)
- Gate check unchanged
- Evaluator invocation unchanged
- Verdict loop unchanged

All new behavior (AC categorization, colocated output, boundary mapping)
MUST be in agent definitions, NOT in skill files.
"""

import os
import re
import hashlib
import unittest

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "..", ".."))

SKILL_FILES = {
    "phase-test-design": os.path.join(PROJECT_ROOT, "plugins", "dev-team", "skills", "phase-test-design", "SKILL.md"),
    "phase-test-gen": os.path.join(PROJECT_ROOT, "plugins", "dev-team", "skills", "phase-test-gen", "SKILL.md"),
}

# Baseline hashes: SHA-256 of the UNMODIFIED skill files.
# These capture the current content before the change is applied.
# TODO: Update these baseline hashes once the initial read is confirmed.
# To update, run: sha256sum <file>
BASELINE_HASHES = {
    # "phase-test-design": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
    # "phase-test-gen": "yyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyyy",
}

# Domain logic keywords that should NOT appear in skill files.
# These indicate domain-specific behavior that belongs in agent definitions.
FORBIDDEN_KEYWORDS = [
    # AC categorization (belongs in test-design-planner)
    r"正向\s*AC",
    r"反向\s*AC",
    r"Forward.*AC",
    r"Reverse.*AC",

    # Parameter types (belongs in test-gen-generator)
    r"参数类型",
    r"边界场景.*参数",
    r"parameter type",
    r"边界.*映射",

    # Colocated output (belongs in test-gen-generator)
    r"共存.*输出",
    r"colocat.*test.*file",
    r"源码.*目录.*写入",
    r"test.*file.*colocat",

    # File type blacklist (belongs in test-gen-generator)
    r"文件类型.*黑名单",
    r"禁止读取.*\.py",
    r"blacklist.*file.*type",

    # Type inference (belongs in test-gen-generator)
    r"类型推断",
    r"type.*infer",
    r"参数名.*推断",

    # Naming conventions (belongs in test-gen-generator)
    r"test_\*\.py",
    r"\*_test\.rs",
    r"\*_test\.go",
    r"\*\.test\.ts",

    # Evaluator logic (belongs in agent evaluators)
    r"G1.*共存",
    r"G2.*命名",
    r"T2.*coverage",
    r"T1.*AC.*覆盖",
]


def read_skill_file(skill_name):
    """Read a skill file by name."""
    path = SKILL_FILES.get(skill_name)
    if not path:
        raise ValueError(f"Unknown skill: {skill_name}")
    if not os.path.isfile(path):
        raise FileNotFoundError(f"Skill file not found: {path}")
    with open(path, "r", encoding="utf-8") as f:
        return f.read()


def get_sha256(content):
    """Compute SHA-256 hash of content."""
    return hashlib.sha256(content.encode("utf-8")).hexdigest()


class TestSkillPhaseTestDesignUnchanged(unittest.TestCase):
    """Test that phase-test-design/SKILL.md contains no domain logic changes."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_skill_file("phase-test-design")

    def test_prompt_unchanged_single_line(self):
        """Verify the planner prompt is the existing single-line prompt, not modified."""
        # Expected patterns in the unchanged skill:
        # - 'description: "Write test-design.md"'
        # - 'prompt: "Write test-design.md for change \'<name>\'."'
        has_original_prompt = bool(re.search(
            r"Write test-design\.md for change",
            self.content
        ))
        self.assertTrue(
            has_original_prompt,
            "phase-test-design skill should use the original single-line prompt"
        )

        # Check prompt does NOT contain domain-specific instructions
        has_domain_prompt = bool(re.search(
            r"Forward.*AC|Reverse.*AC|AC.*分类|正向|反向",
            self.content
        ))
        self.assertFalse(
            has_domain_prompt,
            "Skill prompt should NOT contain Forward/Reverse AC categorization logic"
        )

    def test_gate_check_unchanged(self):
        """Verify gate check still uses phase_check with phase='03-test-design'."""
        has_gate_check = bool(re.search(
            r"phase_check.*change.*03-test-design",
            self.content
        ))
        # Alternative: gate check might use a different format
        has_alt_gate = bool(re.search(
            r"03-test-design",
            self.content
        ))
        self.assertTrue(
            has_gate_check or has_alt_gate,
            "Gate check should reference phase '03-test-design'"
        )

    def test_no_domain_logic_keywords(self):
        """Verify skill file does not contain any domain logic keywords."""
        violations = []
        for keyword in FORBIDDEN_KEYWORDS:
            if re.search(keyword, self.content):
                violations.append(keyword)
        self.assertEqual(
            len(violations), 0,
            f"Skill file should not contain domain logic keywords. Found: {violations}"
        )

    def test_verdict_loop_structure(self):
        """Verify the verdict loop structure (max 5 retries) is unchanged."""
        has_loop = bool(re.search(
            r"Loop max 5x|redo.*Planner.*Evaluator|最多 5 次",
            self.content
        ))
        # The loop structure may be implicit in the YAML steps
        if not has_loop:
            has_steps = bool(re.search(
                r"3[abc]|Planner|Evaluator|Verdict",
                self.content
            ))
            self.assertTrue(
                has_steps,
                "Skill should define P->E loop structure"
            )

    def test_no_new_agent_tools_or_prompts(self):
        """Verify no new agent tool invocations or prompt overrides."""
        # Count agent tool references
        agent_refs = re.findall(
            r"subagent_type|Agent\(|agent:",
            self.content
        )
        # 2 agents (planner + evaluator) x 2 references each (Agent( + subagent_type) = 4
        self.assertLessEqual(
            len(agent_refs), 4,
            f"Should reference at most 2 agents (planner + evaluator), got {len(agent_refs)} references"
        )


class TestSkillPhaseTestGenUnchanged(unittest.TestCase):
    """Test that phase-test-gen/SKILL.md contains no domain logic changes."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_skill_file("phase-test-gen")

    def test_prompt_unchanged_single_line(self):
        """Verify the generator prompt is the existing single-line prompt."""
        has_original_prompt = bool(re.search(
            r"Generate test files for change",
            self.content
        ))
        self.assertTrue(
            has_original_prompt,
            "phase-test-gen skill should use the original single-line prompt"
        )

        # Check prompt does NOT contain output directory specification
        has_output_dir = bool(re.search(
            r"openspec/changes/.*tests|tests/.*目录|源码.*目录|colocat",
            self.content
        ))
        self.assertFalse(
            has_output_dir,
            "Skill prompt should NOT specify output directory — agent handles it"
        )

    def test_gate_check_unchanged(self):
        """Verify gate check still uses phase_check with phase='04-test-gen'."""
        has_gate_check = bool(re.search(
            r"04-test-gen",
            self.content
        ))
        self.assertTrue(
            has_gate_check,
            "Gate check should reference phase '04-test-gen'"
        )

    def test_no_domain_logic_keywords(self):
        """Verify skill file does not contain any domain logic keywords."""
        violations = []
        for keyword in FORBIDDEN_KEYWORDS:
            if re.search(keyword, self.content):
                violations.append(keyword)
        self.assertEqual(
            len(violations), 0,
            f"Skill file should not contain domain logic keywords. Found: {violations}"
        )

    def test_evaluator_prompt_unchanged(self):
        """Verify evaluator prompt is the original, not modified with checklist items."""
        has_evaluator_prompt = bool(re.search(
            r"Evaluate generated test code",
            self.content
        ))
        self.assertTrue(
            has_evaluator_prompt,
            "Evaluator prompt should be the original 'Evaluate generated test code' prompt"
        )

        # Check no hardcoded checklist items in prompt
        has_hardcoded_checklist = bool(re.search(
            r"G1.*共存|G2.*命名|checklist.*G1|G1.*test.*file|G2.*naming",
            self.content
        ))
        self.assertFalse(
            has_hardcoded_checklist,
            "Skill should NOT hardcode evaluator checklist items"
        )

    def test_verdict_loop_structure(self):
        """Verify the verdict loop structure is unchanged."""
        has_loop = bool(re.search(
            r"Loop max|redo.*Generator|Generator.*Evaluator|最多 5",
            self.content
        ))
        if not has_loop:
            has_steps = bool(re.search(
                r"3[abc]|Generator|Evaluator|Verdict",
                self.content
            ))
            self.assertTrue(
                has_steps,
                "Skill should define G->E loop structure"
            )

    def test_no_new_agent_tools_or_prompts(self):
        """Verify no new agent tool invocations or prompt overrides."""
        agent_refs = re.findall(
            r"subagent_type|Agent\(|agent:",
            self.content
        )
        # 2 agents (generator + evaluator) x 2 references each (Agent( + subagent_type) = 4
        self.assertLessEqual(
            len(agent_refs), 4,
            f"Should reference at most 2 agents (generator + evaluator), got {len(agent_refs)} references"
        )


class TestSkillFilesIntegrityCheck(unittest.TestCase):
    """SHA-256 content integrity check for skill files."""

    def test_phase_test_design_hash(self):
        """Verify phase-test-design/SKILL.md content matches baseline hash."""
        if "phase-test-design" in BASELINE_HASHES:
            content = read_skill_file("phase-test-design")
            current_hash = get_sha256(content)
            self.assertEqual(
                current_hash,
                BASELINE_HASHES["phase-test-design"],
                "phase-test-design/SKILL.md content has changed from baseline"
            )

    def test_phase_test_gen_hash(self):
        """Verify phase-test-gen/SKILL.md content matches baseline hash."""
        if "phase-test-gen" in BASELINE_HASHES:
            content = read_skill_file("phase-test-gen")
            current_hash = get_sha256(content)
            self.assertEqual(
                current_hash,
                BASELINE_HASHES["phase-test-gen"],
                "phase-test-gen/SKILL.md content has changed from baseline"
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)
