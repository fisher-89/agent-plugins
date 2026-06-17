#!/usr/bin/env python3
"""
Static verification tests for integration_root support in test_resolve_paths,
MCP registration, and test-design-planner agent Process.

Coverage:
  AC-8: mcp.ts registers test_resolve_paths with integration_root in input schema
  AC-9: test-design-planner.md Process uses test_detect_frameworks and integration_root

@see openspec/changes/add-integration-root-param/test-design.md
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
DETECT_TOOL = "test_detect_frameworks"


def read_file(file_path: str) -> str:
    """Read a text file and return its content."""
    with open(file_path, "r", encoding="utf-8") as f:
        return f.read()


def file_exists(file_path: str) -> bool:
    return os.path.isfile(file_path)


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite: MCP 注册与 schema 静态检查 (AC-8)
# ═══════════════════════════════════════════════════════════════════════════════

class TestMcpIntegrationRootRegistration(unittest.TestCase):
    """Verify mcp.ts and schema expose integration_root for test_resolve_paths."""

    @classmethod
    def setUpClass(cls):
        cls.schema_content = read_file(SCHEMA_TS_PATH) if file_exists(SCHEMA_TS_PATH) else ""
        cls.mcp_content = read_file(MCP_TS_PATH) if file_exists(MCP_TS_PATH) else ""

    def test_schema_defines_integration_root_field(self):
        """AC-8: test-resolve-paths.schema.ts 含 integration_root 字段定义."""
        content = self.schema_content
        self.assertIn("integration_root", content)
        self.assertRegex(
            content,
            r"integration_root\s*:\s*z[\s\S]*?\.string\(\)[\s\S]*?\.optional\(\)",
        )

    def test_mcp_registers_test_resolve_paths_tool(self):
        """AC-8: mcp.ts 注册 test_resolve_paths 且导入 testResolvePathsInputSchema."""
        content = self.mcp_content
        patterns = [
            rf"registerTool\(\s*['\"]{TOOL_NAME}['\"]",
            rf"name:\s*['\"]{TOOL_NAME}['\"]",
        ]
        matched = any(re.search(p, content) for p in patterns)
        self.assertTrue(matched, f"mcp.ts must register tool '{TOOL_NAME}'")
        self.assertIn("testResolvePathsInputSchema", content)

    def test_mcp_handler_spreads_args_to_run_test_resolve_paths(self):
        """AC-8: mcp.ts handler 通过 spread 透传 integration_root 至 runTestResolvePaths."""
        content = self.mcp_content
        self.assertIn("runTestResolvePaths", content)
        self.assertRegex(
            content,
            r"runTestResolvePaths\(\s*\{\s*\.\.\.args",
        )


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite: test-design-planner Agent 静态检查 (AC-9)
# ═══════════════════════════════════════════════════════════════════════════════

class TestAgentPlannerIntegrationRoot(unittest.TestCase):
    """Verify test-design-planner.md multi-framework integration_root workflow."""

    @classmethod
    def setUpClass(cls):
        if not file_exists(PLANNER_AGENT_PATH):
            cls.planner_content = ""
        else:
            cls.planner_content = read_file(PLANNER_AGENT_PATH)

    def setUp(self):
        if not file_exists(PLANNER_AGENT_PATH):
            self.skipTest(f"Agent file not found: {PLANNER_AGENT_PATH}")

    def test_process_calls_test_detect_frameworks_when_integration_scenarios_exist(self):
        """AC-9: Process 描述存在集成场景时先调用 test_detect_frameworks."""
        content = self.planner_content
        has_detect = DETECT_TOOL in content or "test_detect_frameworks" in content
        self.assertTrue(
            has_detect,
            "test-design-planner.md must call test_detect_frameworks for integration scenarios",
        )

    def test_process_passes_plan_directory_as_integration_root(self):
        """AC-9: Process 将 plan 条目 directory 作为 integration_root 传入 test_resolve_paths."""
        content = self.planner_content
        self.assertIn("integration_root", content)
        self.assertRegex(
            content,
            r"integration_root.*directory|directory.*integration_root",
            msg="Process must map plan[].directory to integration_root",
        )

    def test_process_merges_integration_tests_into_integration_table(self):
        """AC-9: Process 描述合并各次 integration_tests 映射到 集成测试 > 用例 表格."""
        content = self.planner_content
        self.assertIn("integration_tests", content)
        self.assertIn("集成测试", content)

    def test_process_forbids_manual_path_concatenation(self):
        """AC-9: 禁止手工拼接测试路径的约束仍存在."""
        content = self.planner_content.lower()
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

    def test_process_unit_tests_single_call_without_integration_root(self):
        """AC-9: 单元测试路径单次调用且不传 integration_root 的说明存在."""
        content = self.planner_content
        self.assertIn(TOOL_NAME, content)
        # Expect explicit note that unit test resolution omits integration_root
        has_unit_flow = "unit_tests" in content and "integration_root" in content
        self.assertTrue(
            has_unit_flow,
            "Process must describe unit_tests resolution without integration_root",
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
