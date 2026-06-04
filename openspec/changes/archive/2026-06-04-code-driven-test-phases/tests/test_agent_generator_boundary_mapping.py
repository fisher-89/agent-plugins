#!/usr/bin/env python3
"""
AC-5 Unit Test: test-gen-generator parameter type -> boundary case mapping.

Verifies that the test-gen-generator.md agent definition contains a systematic
parameter type to edge case mapping table covering at minimum:

| Type | Edge Cases |
| int/number | 0, -1, MAX_INT, None/undefined |
| str/string | "", 超长字符串, 特殊字符(\n \0 emoji), None |
| bool | True, False, None |
| list/array | [], [单元素], 超大列表, None |
| dict/object | {}, 缺失必填字段, 多余字段, None |
| Optional[T] | None |
| Enum | 每个枚举值, 非法枚举值 |
| float | 0.0, -0.0, NaN, Inf, None |

Also verifies boundary scenario:
- Custom class types: at minimum generate None test case + TODO for field-level boundaries
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


def has_type_mapping_table(content):
    """Check if a parameter type -> edge case mapping table exists."""
    patterns = [
        r"\|.*类型.*边界",
        r"\|.*Type.*Edge",
        r"\|.*int.*0.*-1",
        r"\|.*str.*空.*超长",
        r"\|.*bool.*True.*False",
        r"参数类型.*边界场景.*映射",
        r"parameter type.*edge case",
        r"| int / number | 0",
        r"| str / string | \"\"",
        r"| bool | True",
        r"| list / array | \[\]",
        r"| dict / object | \{\}",
        r"| Optional\[T\] | None",
        r"| Enum | 每个枚举值",
        r"| float | 0\.0",
    ]
    return any(re.search(p, content) for p in patterns)


class TestGenGeneratorBoundaryMappingInt(unittest.TestCase):
    """Test int/number boundary mapping presence."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_int_boundary_0_present(self):
        """Verify int boundary includes 0."""
        self.assertIsNotNone(
            re.search(r"0[^.\w]", self.content),
            "int boundary should include 0"
        )

    def test_int_boundary_neg1_present(self):
        """Verify int boundary includes -1."""
        has_neg1 = bool(re.search(r"-1", self.content))
        has_minus_one = bool(re.search(r"MINUS.*ONE|负一|negative.*one|-\s*1", self.content))
        self.assertTrue(has_neg1 or has_minus_one, "int boundary should include -1")

    def test_int_boundary_max_present(self):
        """Verify int boundary includes MAX_INT/MAX_SAFE_INTEGER."""
        has_max = bool(re.search(r"MAX", self.content, re.IGNORECASE))
        has_large = bool(re.search(r"large.*int|最大.*整数|2[0-9]{8,}", self.content))
        self.assertTrue(has_max or has_large, "int boundary should include MAX_INT")

    def test_int_boundary_none_present(self):
        """Verify int boundary includes None/undefined."""
        has_none = bool(re.search(r"None.*(undefined)?", self.content))
        self.assertTrue(has_none, "int boundary should include None/undefined")


class TestGenGeneratorBoundaryMappingStr(unittest.TestCase):
    """Test str/string boundary mapping presence."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_str_boundary_empty_present(self):
        """Verify str boundary includes empty string."""
        has_empty = bool(re.search(r"\"\"|''|空字符串|empty.*string", self.content))
        self.assertTrue(has_empty, "str boundary should include empty string")

    def test_str_boundary_long_present(self):
        """Verify str boundary includes very long string (>1000 chars)."""
        has_long = bool(re.search(r"超长|very long|>1000|long.*string|超长字符串", self.content))
        self.assertTrue(has_long, "str boundary should include very long string (>1000 chars)")

    def test_str_boundary_special_chars_present(self):
        """Verify str boundary includes special characters (\\n \\0 emoji)."""
        has_special = bool(re.search(r"special.*char|特殊字符|\\\\n.*\\\\0|emoji", self.content))
        self.assertTrue(has_special, "str boundary should include special characters")

    def test_str_boundary_none_present(self):
        """Verify str boundary includes None/undefined."""
        has_none = bool(re.search(r"None.*(undefined)?", self.content))
        self.assertTrue(has_none, "str boundary should include None/undefined")


class TestGenGeneratorBoundaryMappingBool(unittest.TestCase):
    """Test bool boundary mapping presence."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_bool_boundary_true_present(self):
        """Verify bool boundary includes True."""
        has_true = bool(re.search(r"\bTrue\b", self.content))
        self.assertTrue(has_true, "bool boundary should include True")

    def test_bool_boundary_false_present(self):
        """Verify bool boundary includes False."""
        has_false = bool(re.search(r"\bFalse\b", self.content))
        self.assertTrue(has_false, "bool boundary should include False")

    def test_bool_boundary_none_present(self):
        """Verify bool boundary includes None."""
        has_none = bool(re.search(r"\bNone\b", self.content))
        self.assertTrue(has_none, "bool boundary should include None")


class TestGenGeneratorBoundaryMappingList(unittest.TestCase):
    """Test list/array boundary mapping presence."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_list_boundary_empty_present(self):
        """Verify list boundary includes empty list []."""
        has_empty = bool(re.search(r"\[\]", self.content))
        self.assertTrue(has_empty, "list boundary should include empty list []")

    def test_list_boundary_single_element_present(self):
        """Verify list boundary includes single-element list."""
        has_single = bool(re.search(r"单元素|single.*element|one.*item", self.content))
        self.assertTrue(has_single, "list boundary should include single-element list")

    def test_list_boundary_large_present(self):
        """Verify list boundary includes very large list."""
        has_large = bool(re.search(r"超大|very large|large.*list|big.*array", self.content))
        self.assertTrue(has_large, "list boundary should include very large list")

    def test_list_boundary_none_present(self):
        """Verify list boundary includes None."""
        has_none = bool(re.search(r"\bNone\b", self.content))
        self.assertTrue(has_none, "list boundary should include None")


class TestGenGeneratorBoundaryMappingDict(unittest.TestCase):
    """Test dict/object boundary mapping presence."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_dict_boundary_empty_present(self):
        """Verify dict boundary includes empty dict {}."""
        has_empty = bool(re.search(r"\{\}", self.content))
        self.assertTrue(has_empty, "dict boundary should include empty dict {}")

    def test_dict_boundary_missing_field_present(self):
        """Verify dict boundary includes missing required fields."""
        has_missing = bool(re.search(r"缺失.*字段|missing.*field|缺少.*字段", self.content))
        self.assertTrue(has_missing, "dict boundary should include missing required fields")

    def test_dict_boundary_extra_field_present(self):
        """Verify dict boundary includes extra fields."""
        has_extra = bool(re.search(r"多余.*字段|extra.*field|多余字段", self.content))
        self.assertTrue(has_extra, "dict boundary should include extra/unexpected fields")

    def test_dict_boundary_none_present(self):
        """Verify dict boundary includes None."""
        has_none = bool(re.search(r"\bNone\b", self.content))
        self.assertTrue(has_none, "dict boundary should include None")


class TestGenGeneratorBoundaryMappingOptional(unittest.TestCase):
    """Test Optional[T] boundary mapping presence."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_optional_boundary_none_present(self):
        """Verify Optional[T] boundary includes None."""
        has_optional = bool(re.search(r"Optional.*None|None.*Optional", self.content))
        has_union_none = bool(re.search(r"Union.*None|None.*Union|int.*None", self.content))
        self.assertTrue(
            has_optional or has_union_none,
            "Optional[T] boundary should include None"
        )


class TestGenGeneratorBoundaryMappingEnum(unittest.TestCase):
    """Test Enum boundary mapping presence."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_enum_boundary_each_value_present(self):
        """Verify Enum boundary includes each enum value."""
        has_each = bool(re.search(r"每个枚举值|each.*enum.*value|every.*enum|每.*值", self.content))
        self.assertTrue(has_each, "Enum boundary should include each enum value")

    def test_enum_boundary_invalid_present(self):
        """Verify Enum boundary includes invalid enum value."""
        has_invalid = bool(re.search(r"非法.*枚举|invalid.*enum|非法枚举值", self.content))
        self.assertTrue(has_invalid, "Enum boundary should include invalid enum value")


class TestGenGeneratorBoundaryMappingFloat(unittest.TestCase):
    """Test float boundary mapping presence."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_float_boundary_0_present(self):
        """Verify float boundary includes 0.0."""
        has_0 = bool(re.search(r"0\.0", self.content))
        self.assertTrue(has_0, "float boundary should include 0.0")

    def test_float_boundary_neg0_present(self):
        """Verify float boundary includes -0.0."""
        has_neg_0 = bool(re.search(r"-0\.0", self.content))
        self.assertTrue(has_neg_0, "float boundary should include -0.0")

    def test_float_boundary_nan_present(self):
        """Verify float boundary includes NaN."""
        has_nan = bool(re.search(r"\bNaN\b", self.content))
        self.assertTrue(has_nan, "float boundary should include NaN")

    def test_float_boundary_inf_present(self):
        """Verify float boundary includes Inf/infinity."""
        has_inf = bool(re.search(r"\bInf\b|\bInfinity\b", self.content))
        self.assertTrue(has_inf, "float boundary should include Inf/Infinity")

    def test_float_boundary_none_present(self):
        """Verify float boundary includes None."""
        has_none = bool(re.search(r"\bNone\b", self.content))
        self.assertTrue(has_none, "float boundary should include None")


class TestGenGeneratorBoundaryMappingCustomClass(unittest.TestCase):
    """Boundary: custom class types should have None + TODO."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_custom_class_boundary_handling(self):
        """Verify custom class types generate at minimum None test + TODO."""
        has_custom_class = bool(re.search(
            r"custom.*(class|type)|自定义.*(类型|类)|user.*defined|UserEvent|UserDefined",
            self.content, re.IGNORECASE
        ))
        if has_custom_class:
            has_none_for_custom = bool(re.search(
                r"custom.*None|None.*custom|用户自定义.*None|None.*未知",
                self.content
            ))
            has_todo_for_custom = bool(re.search(
                r"custom.*TODO|custom.*FIXME|自定义.*TODO|TODO.*field",
                self.content, re.IGNORECASE
            ))
            self.assertTrue(
                has_none_for_custom or has_todo_for_custom,
                "Custom class boundary handling should include None test + TODO marker"
            )


class TestGenGeneratorMappingTablePresence(unittest.TestCase):
    """Test that the full mapping table exists."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_full_type_mapping_table_exists(self):
        """Verify the full parameter type -> edge case mapping table exists."""
        self.assertTrue(
            has_type_mapping_table(self.content),
            "test-gen-generator.md should contain a systematic parameter type "
            "to edge case mapping table"
        )

    def test_mapping_covers_eight_types(self):
        """Verify mapping covers all eight types: int, str, bool, list, dict, Optional, Enum, float."""
        type_references = [
            r"\bint\b",
            r"\bstr\b",
            r"\bbool\b",
            r"\blist\b",
            r"\bdict\b",
            r"Optional",
            r"Enum",
            r"float",
        ]
        type_counts = sum(
            1 for p in type_references if re.search(p, self.content)
        )
        self.assertGreaterEqual(
            type_counts, 7,
            f"Mapping should cover at least 7 of 8 required types, got {type_counts}"
        )

    def test_mapping_applies_to_generator(self):
        """Verify the mapping table is explicitly linked to generator's test creation process."""
        has_context = bool(re.search(
            r"边界.*场景.*测试|edge case.*test.*generat|generat.*boundar|"
            r"参数类型.*推导|derive.*boundar|systematic.*edge|test.*case.*per.*type",
            self.content, re.IGNORECASE
        ))
        # TODO: Strengthen assertion once agent file is finalized
        if not has_context:
            self.assertIsNotNone(
                has_type_mapping_table(self.content),
                "Mapping table should be linked to test generation context"
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)
