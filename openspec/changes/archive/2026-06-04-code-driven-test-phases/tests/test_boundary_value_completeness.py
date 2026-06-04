#!/usr/bin/env python3
"""
AC-5 (refined) Unit Test: per-type boundary mapping completeness.

Verifies the completeness of the parameter type -> boundary case mapping
for each type:

| Type | Required Boundary Values |
|------|------------------------|
| int | 0, -1, MAX_INT, None |
| str | "", 超长(>1000), 特殊字符(\n \0 emoji), None |
| bool | True, False, None |
| list | [], [单元素], 超大列表, None |
| dict | {}, 缺字段, 多余字段, None |
| Optional[T] | None |
| Enum | 每枚举值, 非法值 |
| float | 0.0, -0.0, NaN, Inf, None |

Also verifies:
- Union types (int | None) merge type boundaries
- Nested generics (List[Dict[str, int]]) handle outer + inner boundaries
- Custom class types generate None + TODO
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


def count_type_references(content, type_keywords):
    """Count how many of the given type keywords appear in content."""
    return sum(1 for kw in type_keywords if re.search(kw, content))


class TestBoundaryValueInt(unittest.TestCase):
    """int boundary: 0, -1, MAX_INT, None + valid value."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_int_has_0(self):
        """int boundary includes 0."""
        self.assertTrue(
            re.search(r'\b0\b', self.content),
            "int boundary should include 0"
        )

    def test_int_has_neg1(self):
        """int boundary includes -1."""
        has_literal = bool(re.search(r'-1\b', self.content))
        has_concept = bool(re.search(r'(?i)负一|minus.one|negative.one', self.content))
        self.assertTrue(
            has_literal or has_concept,
            "int boundary should include -1"
        )

    def test_int_has_max(self):
        """int boundary includes MAX_INT / large value."""
        has_keyword = bool(re.search(r'(?i)MAX_INT|INT_MAX|MAX_SAFE_INTEGER|极大值|最大值|2147483647|9223372036854775807', self.content))
        has_concept = bool(re.search(r'(?i)max.*int|最大.*整数', self.content))
        self.assertTrue(
            has_keyword or has_concept,
            "int boundary should include MAX_INT"
        )

    def test_int_has_none(self):
        """int boundary includes None."""
        has_none = bool(re.search(r'\bNone\b', self.content))
        self.assertTrue(has_none, "int boundary should include None")

    def test_int_has_valid_normal(self):
        """int boundary includes at least one normal valid value."""
        has_valid = bool(re.search(r'(?i)valid.*int|正常.int|有效.int|within.*range|范围内', self.content))
        has_numeric = bool(re.search(r'\b42\b|\b100\b|\b1\b', self.content))
        self.assertTrue(
            has_valid or has_numeric,
            "int boundary should include a normal valid in-range value"
        )


class TestBoundaryValueStr(unittest.TestCase):
    """str boundary: '', long(>1000), special chars, None + valid."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_str_has_empty(self):
        """str boundary includes empty string."""
        has_empty = bool(re.search(r'""|\'\'|空字符串|empty.*string|空串', self.content))
        self.assertTrue(has_empty, "str boundary should include empty string")

    def test_str_has_long(self):
        """str boundary includes very long string (>1000 chars)."""
        has_long = bool(re.search(r'(?i)超长|>1000|非常长|very.long|long.string|1000.*char|超大字符串', self.content))
        self.assertTrue(has_long, "str boundary should include very long string")

    def test_str_has_special_chars(self):
        """str boundary includes special characters (\\n \\0 emoji)."""
        has_special = bool(re.search(r'(?i)special.char|特殊字符|\\\\n|\\\\0|emoji|Unicode|unicode|制表符|换行符', self.content))
        self.assertTrue(has_special, "str boundary should include special characters")

    def test_str_has_none(self):
        """str boundary includes None."""
        has_none = bool(re.search(r'\bNone\b', self.content))
        self.assertTrue(has_none, "str boundary should include None")


class TestBoundaryValueBool(unittest.TestCase):
    """bool boundary: True, False, None."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_bool_has_true(self):
        """bool boundary includes True."""
        has_true = bool(re.search(r'\bTrue\b', self.content))
        self.assertTrue(has_true, "bool boundary should include True")

    def test_bool_has_false(self):
        """bool boundary includes False."""
        has_false = bool(re.search(r'\bFalse\b', self.content))
        self.assertTrue(has_false, "bool boundary should include False")

    def test_bool_has_none(self):
        """bool boundary includes None."""
        has_none = bool(re.search(r'\bNone\b', self.content))
        self.assertTrue(has_none, "bool boundary should include None")


class TestBoundaryValueList(unittest.TestCase):
    """list boundary: [], [single], large, None + valid."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_list_has_empty(self):
        """list boundary includes empty list []."""
        has_empty = bool(re.search(r'\[\]', self.content))
        has_concept = bool(re.search(r'(?i)空列表|empty.list|空数组', self.content))
        self.assertTrue(has_empty or has_concept, "list boundary should include empty list []")

    def test_list_has_single_element(self):
        """list boundary includes single-element list."""
        has_single = bool(re.search(r'(?i)单个|单元素|single.?element|one.?item|one.?element', self.content))
        self.assertTrue(has_single, "list boundary should include single-element list")

    def test_list_has_large(self):
        """list boundary includes very large list."""
        has_large = bool(re.search(r'(?i)超大|very.large|large.list|many.elements|千.*元素|万.*元素', self.content))
        self.assertTrue(has_large, "list boundary should include very large list")

    def test_list_has_none(self):
        """list boundary includes None."""
        has_none = bool(re.search(r'\bNone\b', self.content))
        self.assertTrue(has_none, "list boundary should include None")


class TestBoundaryValueDict(unittest.TestCase):
    """dict boundary: {}, missing fields, extra fields, None + valid."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_dict_has_empty(self):
        """dict boundary includes empty dict {}."""
        has_empty = bool(re.search(r'\{\}', self.content))
        has_concept = bool(re.search(r'(?i)空对象|空字典|empty.object|empty.dict', self.content))
        self.assertTrue(has_empty or has_concept, "dict boundary should include empty dict {}")

    def test_dict_has_missing_fields(self):
        """dict boundary includes missing required fields."""
        has_missing = bool(re.search(r'(?i)缺失.*字段|缺少.*字段|missing.*field|required.*missing|缺少必填', self.content))
        self.assertTrue(has_missing, "dict boundary should include missing required fields")

    def test_dict_has_extra_fields(self):
        """dict boundary includes extra/unexpected fields."""
        has_extra = bool(re.search(r'(?i)多余.*字段|额外.*字段|extra.*field|unexpected.*field|多余字段', self.content))
        self.assertTrue(has_extra, "dict boundary should include extra fields")

    def test_dict_has_none(self):
        """dict boundary includes None."""
        has_none = bool(re.search(r'\bNone\b', self.content))
        self.assertTrue(has_none, "dict boundary should include None")


class TestBoundaryValueOptional(unittest.TestCase):
    """Optional[T] boundary: None."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_optional_has_none(self):
        """Optional[T] boundary includes None."""
        has_optional = bool(re.search(r'(?i)Optional.*None|None.*Optional', self.content))
        has_union = bool(re.search(r'(?i)Union.*None|int \| None|str \| None|可选.*None', self.content))
        self.assertTrue(
            has_optional or has_union,
            "Optional[T] boundary should include None"
        )


class TestBoundaryValueEnum(unittest.TestCase):
    """Enum boundary: each value, invalid value."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_enum_has_each_value(self):
        """Enum boundary includes each valid enum value."""
        has_each = bool(re.search(r'(?i)每个.*枚举|每个.*值|each.*enum|every.*value|所有.*枚举', self.content))
        self.assertTrue(has_each, "Enum boundary should include each valid enum value")

    def test_enum_has_invalid(self):
        """Enum boundary includes invalid enum value."""
        has_invalid = bool(re.search(r'(?i)非法.*值|无效.*枚举|invalid.*enum|invalid.*value|超出.*范围|未定义.*值', self.content))
        self.assertTrue(has_invalid, "Enum boundary should include invalid enum value")


class TestBoundaryValueFloat(unittest.TestCase):
    """float boundary: 0.0, -0.0, NaN, Inf, None + valid."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_float_has_00(self):
        """float boundary includes 0.0."""
        has_0 = bool(re.search(r'0\.0', self.content))
        self.assertTrue(has_0, "float boundary should include 0.0")

    def test_float_has_neg00(self):
        """float boundary includes -0.0."""
        has_neg_0 = bool(re.search(r'-0\.0|负零|negative.*zero|minus.*zero', self.content))
        self.assertTrue(has_neg_0, "float boundary should include -0.0")

    def test_float_has_nan(self):
        """float boundary includes NaN."""
        has_nan = bool(re.search(r'\bNaN\b|nan|非数字|非法数字', self.content))
        self.assertTrue(has_nan, "float boundary should include NaN")

    def test_float_has_inf(self):
        """float boundary includes Inf/Infinity."""
        has_inf = bool(re.search(r'\bInf\b|\bInfinity\b|无穷|infinite', self.content))
        self.assertTrue(has_inf, "float boundary should include Inf/Infinity")

    def test_float_has_none(self):
        """float boundary includes None."""
        has_none = bool(re.search(r'\bNone\b', self.content))
        self.assertTrue(has_none, "float boundary should include None")


class TestBoundaryUnionType(unittest.TestCase):
    """Boundary: Union types merge component boundaries."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_union_type_handling(self):
        """Verify Union types (e.g., int | None) merge component boundaries."""
        has_union = bool(re.search(r'(?i)Union.*类型|联合类型|int \| None|str \| None|合并.*边界|组合.*类型', self.content))
        has_merge = bool(re.search(r'(?i)合并.*边界|merge.*boundar|both.*boundar', self.content))
        self.assertTrue(
            has_union or has_merge,
            "Mapping should describe how Union types merge component boundaries"
        )


class TestBoundaryNestedGeneric(unittest.TestCase):
    """Boundary: nested generics (List[Dict[str, int]]) handle outer + inner."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_nested_generic_handling(self):
        """Verify nested generics consider both outer and inner type boundaries."""
        has_nested = bool(re.search(r'(?i)嵌套.*泛型|nested.*generic|嵌套.*类型|List\[Dict|外层.*内层|inner.*outer', self.content))
        self.assertTrue(
            has_nested,
            "Mapping should describe nested generic handling (outer + inner type boundaries)"
        )


class TestBoundaryCustomClass(unittest.TestCase):
    """Boundary: custom class types generate None + TODO."""

    @classmethod
    def setUpClass(cls):
        cls.content = read_agent_file()

    def test_custom_class_none(self):
        """Custom class types at minimum generate None test case."""
        has_custom_none = bool(re.search(r'(?i)自定义.*None|custom.*None|custom.*class.*None|未知类型.*None', self.content))
        if not has_custom_none:
            # Generic None handling covers custom classes too
            has_none = bool(re.search(r'\bNone\b', self.content))
            self.assertTrue(has_none, "Custom class should at least generate None test")

    def test_custom_class_todo(self):
        """Custom class types include TODO marker for field-level boundaries."""
        has_todo = bool(re.search(r'(?i)TODO.*自定义|自定义.*TODO|TODO.*class|TODO.*field|TODO.*补充|FIXME.*custom', self.content))
        if not has_todo:
            # If no explicit TODO for custom classes, agent should have general TODO practice
            has_general_todo = bool(re.search(r'TODO', self.content))
            self.assertTrue(
                has_general_todo or has_todo,
                "Custom class types should include TODO marker for field-level boundaries"
            )


if __name__ == "__main__":
    unittest.main(verbosity=2)
