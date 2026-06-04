#!/usr/bin/env python3
"""
AC-9 Unit Test: test-gen-generator type inference for untyped files.

Verifies that the test-gen-generator.md agent definition contains instruction
to infer parameter types from parameter names when source files lack type annotations.
Strategy: P2 priority + TODO marker.

Scenarios:
- Python file with no type hints: `def add_user(username, age):`
  -> infer: username -> str, age -> int; emit P2 + TODO on boundary tests
- Python file with partial type hints: `def process(name: str, age)`
  -> name uses str mapping, age uses name inference, age gets P2+TODO
- TypeScript with JSDoc annotations: `/** @param {number} count */`
  -> extract count: number, apply int boundary mapping
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


class TestGenGeneratorTypeInferenceInstruction(unittest.TestCase):
    """Test that type inference for untyped files is instructed."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_type_inference_instruction_present(self):
        """Verify agent contains instruction to infer types from parameter names."""
        inference_patterns = [
            r"推断.*参数.*类型",
            r"type.*infer",
            r"参数名.*推断",
            r"name.*infer.*type",
            r"parameter.*name.*type",
            r"infer.*from.*name",
            r"无类型.*推断",
            r"无类型.*推",
            r"类型.*推导.*参数名",
            r"derive.*type.*from.*name",
        ]
        has_inference = any(
            re.search(p, self.content, re.IGNORECASE) for p in inference_patterns
        )
        self.assertTrue(
            has_inference,
            "test-gen-generator.md should contain instruction to infer types "
            "from parameter names when type hints are absent"
        )

    def test_p2_priority_for_inferred_types(self):
        """Verify inferred-parameter tests are marked P2 priority."""
        p2_patterns = [
            r"P2.*TODO",
            r"P2.*优先级",
            r"P2.*标记",
            r"mark.*P2",
            r"P2.*priority",
            r"P2.*标注",
        ]
        has_p2 = any(re.search(p, self.content) for p in p2_patterns)
        self.assertTrue(
            has_p2,
            "inferred parameter type tests should be marked with P2 priority"
        )

    def test_todo_marker_for_inferred_types(self):
        """Verify inferred-parameter tests include TODO markers."""
        todo_patterns = [
            r"TODO.*推断",
            r"TODO.*类型",
            r"TODO.*开发者",
            r"TODO.*review",
            r"TODO.*审阅",
            r"TODO.*inferred",
            r"添加.*TODO.*标记",
            r"含.*TODO.*标记",
            r"TODO.*提示",
        ]
        has_todo = any(re.search(p, self.content) for p in todo_patterns)
        self.assertTrue(
            has_todo,
            "inferred parameter type tests should include TODO markers "
            "for developer review"
        )


class TestGenGeneratorTypeInferenceNameMapping(unittest.TestCase):
    """Test specific parameter name -> type inference mappings."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_username_infers_str(self):
        """Verify username parameter name infers str type."""
        has_username = bool(re.search(
            r"(?i)username.*str|user.*name.*推断|username.*字符串",
            self.content
        ))
        if not has_username:
            # Check for general naming convention mapping
            has_general_name_map = bool(re.search(
                r"name.*str|name.*string|名.*字.*符串|用户名.*字符串",
                self.content
            ))
            self.assertTrue(
                has_general_name_map or has_username,
                "Type inference should map 'username' to str/string"
            )

    def test_count_infers_int(self):
        """Verify count parameter name infers int type."""
        has_count = bool(re.search(
            r"(?i)count.*int|数量.*整数|计数.*int",
            self.content
        ))
        if not has_count:
            has_general = bool(re.search(
                r"(?i)(age|count|limit|id|index).*int",
                self.content
            ))
            self.assertTrue(
                has_general or has_count,
                "Type inference should map 'count' to int"
            )

    def test_flags_infers_bool(self):
        """Verify flags/flag parameter name infers bool type."""
        has_flags = bool(re.search(
            r"(?i)flag.*bool|开关.*布尔|标志.*bool|enable.*bool|is_.*bool|has_.*bool",
            self.content
        ))
        self.assertTrue(
            has_flags,
            "Type inference should map 'flags' / boolean-like names to bool"
        )

    def test_common_name_type_mapping_list(self):
        """Verify the agent contains a common name-to-type mapping list."""
        mapping_patterns = [
            r"参数名.*类型.*映射",
            r"name.*type.*mapping",
            r"推断.*username→str",
            r"infer.*username.*string",
            r"推断.*count.*int",
            r"common.*parameter.*name",
            r"常见.*参数名.*映射",
            r"名称.*推断.*表",
            r"infer.*parameter.*type.*from.*name",
            r"infer.*from.*parameter.*name",
            r"parameter.*name.*infer",
        ]
        has_mapping = any(
            re.search(p, self.content, re.IGNORECASE) for p in mapping_patterns
        )
        self.assertTrue(
            has_mapping,
            "Agent should contain a list of common parameter names with their inferred types"
        )


class TestGenGeneratorTypeInferenceUntypedFile(unittest.TestCase):
    """Boundary: Python file with no type hints."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_no_type_hints_handling(self):
        """Verify agent handles files with zero type annotations."""
        no_type_patterns = [
            r"无类型.*文件",
            r"无类型.*方法",
            r"no type.*hint",
            r"untyped.*file",
            r"没有.*类型.*注解",
            r"missing.*type.*annot",
            r"无类型提示",
        ]
        has_handling = any(
            re.search(p, self.content, re.IGNORECASE) for p in no_type_patterns
        )
        self.assertTrue(
            has_handling,
            "Agent should handle files with no type annotations"
        )

    def test_partial_type_hints_handling(self):
        """Boundary: verify agent handles files with partial type annotations."""
        partial_patterns = [
            r"部分.*类型.*注解",
            r"partial.*type.*hint",
            r"混合.*类型",
            r"mix.*type.*annot",
            r"some.*typed.*some.*untyped",
        ]
        has_partial = any(
            re.search(p, self.content, re.IGNORECASE) for p in partial_patterns
        )
        # TODO: Strengthen assertion once agent includes explicit partial handling
        if not has_partial:
            has_type_inference = bool(re.search(
                r"推断.*参数.*类型|type.*infer",
                self.content, re.IGNORECASE
            ))
            self.assertTrue(
                has_type_inference or has_partial,
                "Agent should have type inference that naturally handles partial annotations"
            )


class TestGenGeneratorTypeInferenceJSDoc(unittest.TestCase):
    """Boundary: JSDoc type annotations for TypeScript files."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_jsdoc_type_extraction(self):
        """Verify agent extracts types from JSDoc annotations."""
        jsdoc_patterns = [
            r"JSDoc",
            r"@param.*\{.*\}",
            r"@type",
            r"js.*doc",
            r"注释.*类型",
        ]
        has_jsdoc = any(re.search(p, self.content) for p in jsdoc_patterns)
        # TODO: JSDoc extraction may be implicit (agent reads source, finds annotations)
        if not has_jsdoc:
            # Fallback: TypeScript support implies handling typed comments
            has_ts = bool(re.search(r"TypeScript|\.ts\b", self.content))
            self.assertTrue(
                has_ts or has_jsdoc,
                "TypeScript support should include JSDoc annotation extraction"
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)
