#!/usr/bin/env python3
"""
Static verification tests for test_resolve_paths integration with
test-design-planner agent and MCP registration.

Coverage:
  AC-11: mcp.ts registers test_resolve_paths with correct schema imports
  AC-12: test-design-planner.md Process calls test_resolve_paths and maps results

@see openspec/changes/add-test-path-resolver-api/test-design.md
"""

import os
import re
import unittest


# ─── Constants ────────────────────────────────────────────────────────────────

PROJECT_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "..", "..", "..", "..")
)

PLANNER_AGENT_PATH = os.path.join(
    PROJECT_ROOT, "plugins", "dev-team", "agents", "test-design-planner.md"
)

MCP_TS_PATH = os.path.join(
    PROJECT_ROOT, "plugins", "dev-team", "bin", "src", "mcp.ts"
)

SCHEMA_TS_PATH = os.path.join(
    PROJECT_ROOT,
    "plugins",
    "dev-team",
    "bin",
    "src",
    "schemas",
    "test-resolve-paths.schema.ts",
)

TOOL_FQN = "mcp__plugin_dev-team_dev-team__test_resolve_paths"
TOOL_NAME = "test_resolve_paths"


def read_file(file_path: str) -> str:
    """Read a text file and return its content."""
    with open(file_path, "r", encoding="utf-8") as f:
        return f.read()


def file_exists(file_path: str) -> bool:
    return os.path.isfile(file_path)


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite: test-design-planner Agent 静态检查 (AC-12)
# ═══════════════════════════════════════════════════════════════════════════════

class TestAgentPlannerResolvePaths(unittest.TestCase):
    """Verify test-design-planner.md references test_resolve_paths correctly."""

    @classmethod
    def setUpClass(cls):
        if not file_exists(PLANNER_AGENT_PATH):
            cls.planner_content = ""
        else:
            cls.planner_content = read_file(PLANNER_AGENT_PATH)

    def setUp(self):
        if not file_exists(PLANNER_AGENT_PATH):
            self.skipTest(f"Agent file not found: {PLANNER_AGENT_PATH}")

    def test_process_contains_test_resolve_paths_call(self):
        """AC-12: Process 包含 test_resolve_paths / MCP FQN 调用步骤."""
        content = self.planner_content
        has_tool_name = TOOL_NAME in content
        has_fqn = TOOL_FQN in content
        self.assertTrue(
            has_tool_name or has_fqn,
            f"test-design-planner.md must reference {TOOL_NAME} or {TOOL_FQN}",
        )

    def test_process_forbids_manual_path_concatenation(self):
        """AC-12: 包含禁止手工拼接测试路径的约束."""
        content = self.planner_content.lower()
        # Expect explicit prohibition of manual path guessing/concatenation
        patterns = [
            r"禁止.*手工",
            r"shall\s+not.*手工",
            r"must\s+not.*手工",
            r"禁止.*拼接",
            r"shall\s+not.*guess",
        ]
        matched = any(re.search(p, content, re.IGNORECASE) for p in patterns)
        self.assertTrue(
            matched,
            "test-design-planner.md should forbid manual test path concatenation",
        )

    def test_process_maps_unit_tests_to_test_file_column(self):
        """AC-12: 包含将 unit_tests 映射到 测试文件 列的说明."""
        content = self.planner_content
        self.assertIn("unit_tests", content)
        self.assertIn("测试文件", content)

    def test_process_maps_integration_tests_to_test_file_column(self):
        """AC-12: 包含将 integration_tests 映射到 测试文件 列的说明."""
        content = self.planner_content
        self.assertIn("integration_tests", content)

    def test_process_maps_errors_to_untestable_section(self):
        """AC-12: 包含 errors → 不可测试项 的处理说明."""
        content = self.planner_content
        self.assertIn("errors", content)
        self.assertIn("不可测试项", content)


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite: MCP 工具注册静态检查 (AC-11)
# ═══════════════════════════════════════════════════════════════════════════════

class TestMcpToolRegistration(unittest.TestCase):
    """Verify mcp.ts registers test_resolve_paths with correct handler and schemas."""

    @classmethod
    def setUpClass(cls):
        if not file_exists(MCP_TS_PATH):
            cls.mcp_content = ""
        else:
            cls.mcp_content = read_file(MCP_TS_PATH)

    def setUp(self):
        if not file_exists(MCP_TS_PATH):
            self.skipTest(f"MCP source not found: {MCP_TS_PATH}")

    def test_mcp_registers_test_resolve_paths_tool(self):
        """AC-11: mcp.ts 包含 registerTool('test_resolve_paths' 或等效注册."""
        content = self.mcp_content
        patterns = [
            rf"registerTool\(\s*['\"]{TOOL_NAME}['\"]",
            rf"registerTool\(\s*['\"]{TOOL_NAME}['\"]",
            rf"name:\s*['\"]{TOOL_NAME}['\"]",
        ]
        matched = any(re.search(p, content) for p in patterns)
        self.assertTrue(
            matched,
            f"mcp.ts must register tool '{TOOL_NAME}'",
        )

    def test_mcp_handler_calls_run_test_resolve_paths(self):
        """AC-11: mcp.ts handler 调用 runTestResolvePaths."""
        content = self.mcp_content
        self.assertIn("runTestResolvePaths", content)

    def test_mcp_imports_input_schema(self):
        """AC-11: mcp.ts 导入 testResolvePathsInputSchema."""
        content = self.mcp_content
        self.assertIn("testResolvePathsInputSchema", content)

    def test_mcp_imports_output_schema(self):
        """AC-11: mcp.ts 导入 testResolvePathsOutputSchema."""
        content = self.mcp_content
        self.assertIn("testResolvePathsOutputSchema", content)

    def test_schema_file_exists(self):
        """AC-11: test-resolve-paths.schema.ts 文件存在."""
        self.assertTrue(
            file_exists(SCHEMA_TS_PATH),
            f"Schema file expected at: {SCHEMA_TS_PATH}",
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
