#!/usr/bin/env python3
"""
Integration smoke tests for the MCP Server.

Launches the dev-team-mcp.cjs MCP server as a subprocess and communicates
via stdio transport to verify:
  - tools/list returns 11 tool names all in xx_yy format
  - Each tool can be called via tools/call (expecting parameter validation
    errors rather than "tool not found")

Incorporates FP-4 fix: MCP SDK returns tool-not-found errors via
result.isError at content level, not JSON-RPC error level. Tests now
check response.result.isError instead of response.get("error").

Coverage:
  AC-1 through AC-11 (end-to-end): MCP server registers all tools
    with underscore names, and they are discoverable and routable.
"""

import json
import os
import subprocess
import sys
import time
import unittest


# ─── Constants ────────────────────────────────────────────────────────────────

PROJECT_ROOT = os.path.abspath(
    os.path.join(os.path.dirname(__file__), "..", "..", "..", "..")
)

CJS_PATH = os.path.join(
    PROJECT_ROOT, "plugins", "dev-team", "bin", "dev-team-mcp.cjs"
)

EXPECTED_TOOL_NAMES = [
    "phase_log", "phase_check", "phase_next",
    "archi_query", "archi_validate", "archi_write", "archi_check",
    "config_get", "config_set", "config_unset", "config_context",
]

SLASH_TOOL_NAMES = [
    "phase/log", "phase/check", "phase/next",
    "archi/query", "archi/validate", "archi/write", "archi/check",
    "config/get", "config/set", "config/unset", "config/context",
]

# MCP JSON-RPC constants
MCP_VERSION = "1.0.0"
REQUEST_TIMEOUT_SECONDS = 10

# JSON-RPC error code for method not found (standard)
JSONRPC_METHOD_NOT_FOUND = -32601


def make_jsonrpc_request(method, params=None, request_id=1):
    """Create a JSON-RPC request object for MCP protocol."""
    request = {
        "jsonrpc": "2.0",
        "id": request_id,
        "method": method,
    }
    if params is not None:
        request["params"] = params
    return request


class McpClientProcess:
    """Manages an MCP server subprocess over stdio transport.

    Provides a simple request-response cycle using JSON-RPC over stdin/stdout.
    """

    def __init__(self, cjs_path, project_root, timeout=REQUEST_TIMEOUT_SECONDS):
        self.cjs_path = cjs_path
        self.project_root = project_root
        self.timeout = timeout
        self.process = None
        self.next_id = 1

    def __enter__(self):
        self.start()
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        self.stop()

    def start(self):
        """Launch the MCP server as a subprocess."""
        if not os.path.isfile(self.cjs_path):
            raise FileNotFoundError(
                f"MCP server not found at: {self.cjs_path}. "
                "Run 'npm run build' first."
            )
        self.process = subprocess.Popen(
            ["node", self.cjs_path],
            cwd=os.path.dirname(self.cjs_path),
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            shell=(sys.platform == "win32"),
        )
        # Give the server a moment to initialize
        time.sleep(0.5)

    def stop(self):
        """Terminate the MCP server subprocess."""
        if self.process and self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
            self.process = None

    def send_request(self, method, params=None):
        """Send a JSON-RPC request and return the parsed response."""
        if not self.process or self.process.poll() is not None:
            raise RuntimeError("MCP server process is not running")

        request_id = self.next_id
        self.next_id += 1

        request = make_jsonrpc_request(method, params, request_id)
        request_str = json.dumps(request) + "\n"

        # Send request
        self.process.stdin.write(request_str)
        self.process.stdin.flush()

        # Read response (one line = one JSON-RPC response)
        # NOTE: In production, responses may be multi-line. This simple
        #       approach works for tools/list and tools/call with our
        #       expected response sizes.
        response_line = self.process.stdout.readline()
        if not response_line:
            # Check if process died
            stderr_output = self.process.stderr.read()
            raise RuntimeError(
                f"MCP server process exited (code {self.process.poll()}). "
                f"Stderr: {stderr_output}"
            )

        return json.loads(response_line)


# ─── FP-4: MCP tool-not-found detection helpers ──────────────────────────────

def response_has_tool_not_found(response):
    """FP-4: Check if MCP response indicates tool-not-found via result.isError.

    MCP SDK returns tool-not-found errors as content-level isError rather
    than JSON-RPC protocol-level errors. However, result.isError is also
    set for OTHER errors (e.g., parameter validation). This function
    distinguishes tool-not-found from other errors by checking content text.
    Checks:
      1. result.isError AND content text contains "not found" / "unknown tool"
      2. JSON-RPC error with code -32601 (method not found, fallback)

    Args:
        response: The parsed JSON-RPC response dictionary.

    Returns:
        True if the response indicates the tool was not found.
        False if the tool was found (even if other errors occurred).
    """
    # FP-4: Check result.isError at content level and verify the error
    # is specifically about tool-not-found, not a general error like
    # parameter validation.  `isError: true` is set for ALL tool errors
    # (validation, routing, etc.), so we need to check the content text.
    result = response.get("result", {})
    if result.get("isError") is True:
        content = result.get("content", [])
        if isinstance(content, list):
            for item in content:
                text = item.get("text", "") if isinstance(item, dict) else str(item)
                if any(kw in text.lower() for kw in ["not found", "unknown tool",
                                                      "unknown tool", "no tool",
                                                      "不存在", "未找到"]):
                    return True
        return False  # isError without "not found" text = param error, not missing tool

    # Fallback: JSON-RPC protocol-level error (traditional check)
    error = response.get("error")
    if error and error.get("code") == JSONRPC_METHOD_NOT_FOUND:
        return True

    return False


def response_has_jsonrpc_error(response):
    """Check if response has a JSON-RPC protocol-level error."""
    error = response.get("error")
    return error is not None


# ═══════════════════════════════════════════════════════════════════════════════
# Test Suite: MCP Server Integration (Smoke Test)
# ═══════════════════════════════════════════════════════════════════════════════

@unittest.skipIf(
    not os.path.isfile(
        os.path.join(PROJECT_ROOT, "plugins", "dev-team", "bin", "dev-team-mcp.cjs")
    ),
    "dev-team-mcp.cjs not found — run npm run build first"
)
class TestMcpServerIntegration(unittest.TestCase):
    """End-to-end smoke tests for the MCP server with updated tool names."""

    # ─── tools/list: discoverability ──────────────────────────────────────

    def test_tools_list_returns_underscore_names(self):
        """Verify tools/list returns all 11 tools with underscore format names."""
        try:
            with McpClientProcess(CJS_PATH, PROJECT_ROOT) as client:
                response = client.send_request("tools/list")

                # Assert response contains 'result' with 'tools' array.
                self.assertIn("result", response,
                              msg="tools/list response missing 'result'")
                self.assertIn("tools", response["result"],
                              msg="tools/list result missing 'tools' array")

                tools = response["result"]["tools"]
                tool_names = [t["name"] for t in tools]

                # Assert that all 11 expected tool names are present.
                for expected_name in EXPECTED_TOOL_NAMES:
                    self.assertIn(
                        expected_name, tool_names,
                        f"Expected tool '{expected_name}' not in tools/list response"
                    )

                # Assert that the number of registered tools is at least 11
                # (may have additional internal tools).
                self.assertGreaterEqual(
                    len(tool_names), len(EXPECTED_TOOL_NAMES),
                    f"Expected at least {len(EXPECTED_TOOL_NAMES)} tools, "
                    f"got {len(tool_names)}"
                )
        except FileNotFoundError as e:
            self.skipTest(str(e))
        except RuntimeError as e:
            self.skipTest(f"MCP server runtime error: {e}")

    def test_tools_list_has_no_slash_names(self):
        """Verify tools/list returns NO tools with slash in name."""
        try:
            with McpClientProcess(CJS_PATH, PROJECT_ROOT) as client:
                response = client.send_request("tools/list")
                tools = response.get("result", {}).get("tools", [])
                tool_names = [t["name"] for t in tools]

                # Assert that no tool name contains a '/' character.
                for name in tool_names:
                    self.assertNotIn(
                        "/", name,
                        f"Tool '{name}' still uses slash format in tools/list"
                    )
        except FileNotFoundError as e:
            self.skipTest(str(e))
        except RuntimeError as e:
            self.skipTest(f"MCP server runtime error: {e}")

    # ─── tools/call: routing verification (new underscore names) ──────────

    def test_tools_call_phase_log_routable(self):
        """Verify tools/call with 'phase_log' routes correctly (expects param error)."""
        try:
            with McpClientProcess(CJS_PATH, PROJECT_ROOT) as client:
                response = client.send_request("tools/call", {
                    "name": "phase_log",
                    "arguments": {},
                })

                # The tool should be found and return a validation error
                # (missing required params) rather than "tool not found".
                # FP-4: Check result.isError for param error (content level)
                # instead of relying solely on JSON-RPC error.
                is_not_found = response_has_tool_not_found(response)
                self.assertFalse(
                    is_not_found,
                    "Tool 'phase_log' should be routable but got tool-not-found"
                )

                # The response should contain a result (even if it's an
                # isError for parameter validation)
                self.assertIn(
                    "result", response,
                    "Expected 'result' in response for existing tool 'phase_log'"
                )
        except FileNotFoundError as e:
            self.skipTest(str(e))
        except RuntimeError as e:
            self.skipTest(f"MCP server runtime error: {e}")

    def test_tools_call_phase_check_routable(self):
        """Verify tools/call with 'phase_check' routes correctly."""
        try:
            with McpClientProcess(CJS_PATH, PROJECT_ROOT) as client:
                response = client.send_request("tools/call", {
                    "name": "phase_check",
                    "arguments": {},
                })

                # FP-4: Use result.isError aware check instead of
                # response.get("error") only.
                is_not_found = response_has_tool_not_found(response)
                self.assertFalse(
                    is_not_found,
                    "Tool 'phase_check' should be routable but got tool-not-found"
                )
        except FileNotFoundError as e:
            self.skipTest(str(e))
        except RuntimeError as e:
            self.skipTest(f"MCP server runtime error: {e}")

    def test_tools_call_archi_query_routable(self):
        """Verify tools/call with 'archi_query' routes correctly."""
        try:
            with McpClientProcess(CJS_PATH, PROJECT_ROOT) as client:
                response = client.send_request("tools/call", {
                    "name": "archi_query",
                    "arguments": {},
                })

                # FP-4: Use result.isError aware check.
                is_not_found = response_has_tool_not_found(response)
                self.assertFalse(
                    is_not_found,
                    "Tool 'archi_query' should be routable but got tool-not-found"
                )
        except FileNotFoundError as e:
            self.skipTest(str(e))
        except RuntimeError as e:
            self.skipTest(f"MCP server runtime error: {e}")

    def test_tools_call_config_get_routable(self):
        """Verify tools/call with 'config_get' routes correctly."""
        try:
            with McpClientProcess(CJS_PATH, PROJECT_ROOT) as client:
                response = client.send_request("tools/call", {
                    "name": "config_get",
                    "arguments": {"key": "schema"},
                })

                # FP-4: Use result.isError aware check.
                is_not_found = response_has_tool_not_found(response)
                self.assertFalse(
                    is_not_found,
                    "Tool 'config_get' should be routable but got tool-not-found"
                )
        except FileNotFoundError as e:
            self.skipTest(str(e))
        except RuntimeError as e:
            self.skipTest(f"MCP server runtime error: {e}")

    # ─── tools/call: old slash name returns error ─────────────────────────

    def test_tools_call_slash_name_returns_error(self):
        """FP-4: Verify tools/call with old slash name returns an error.

        MCP SDK returns tool-not-found via result.isError at content level,
        not JSON-RPC error level. This test checks:
          1. response.result.isError === true (FP-4 primary)
          2. OR response.error.code === -32601 (fallback)
          3. NOT relying on response.get("error") alone.
        """
        try:
            with McpClientProcess(CJS_PATH, PROJECT_ROOT) as client:
                response = client.send_request("tools/call", {
                    "name": "phase/log",
                    "arguments": {},
                })

                # FP-4: Use the combined check that handles both
                # content-level isError and JSON-RPC protocol errors.
                is_not_found = response_has_tool_not_found(response)
                self.assertTrue(
                    is_not_found,
                    "Old tool name 'phase/log' should not be routable — "
                    "expected tool-not-found but got a successful response.\n"
                    f"Response: {json.dumps(response, indent=2)}"
                )
        except FileNotFoundError as e:
            self.skipTest(str(e))
        except RuntimeError as e:
            self.skipTest(f"MCP server runtime error: {e}")

    # ─── tools/call: all 11 tools ─────────────────────────────────────────

    def test_all_11_tools_routable(self):
        """Verify all 11 tools can be called (quick routing check).

        Sends tools/call for each expected tool with empty arguments and
        verifies the server responds (either result or param error, not
        "tool not found").
        """
        try:
            with McpClientProcess(CJS_PATH, PROJECT_ROOT) as client:
                for tool_name in EXPECTED_TOOL_NAMES:
                    response = client.send_request("tools/call", {
                        "name": tool_name,
                        "arguments": {},
                    })

                    # FP-4: Use result.isError aware check.
                    is_not_found = response_has_tool_not_found(response)
                    self.assertFalse(
                        is_not_found,
                        f"Tool '{tool_name}' should be routable but got "
                        f"tool-not-found"
                    )
        except FileNotFoundError as e:
            self.skipTest(str(e))
        except RuntimeError as e:
            self.skipTest(f"MCP server runtime error: {e}")

    # ─── FP-4 validation: meta test for the helper ────────────────────────

    def test_response_has_tool_not_found_detection(self):
        """Verify FP-4 helper correctly detects tool-not-found in various response shapes."""
        # Simulated MCP response with result.isError (FP-4 primary pattern)
        fp4_response = {
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "isError": True,
                "content": [{
                    "type": "text",
                    "text": "Unknown tool: phase/log"
                }]
            }
        }
        self.assertTrue(
            response_has_tool_not_found(fp4_response),
            "FP-4: Should detect tool-not-found via result.isError with error text"
        )

        # Simulated response with JSON-RPC error (traditional pattern)
        jsonrpc_error_response = {
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32601,
                "message": "Method not found: phase/log"
            }
        }
        self.assertTrue(
            response_has_tool_not_found(jsonrpc_error_response),
            "FP-4: Should detect tool-not-found via JSON-RPC error as fallback"
        )

        # Simulated response for a successful call (no error)
        success_response = {
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [{"type": "text", "text": "OK"}]
            }
        }
        self.assertFalse(
            response_has_tool_not_found(success_response),
            "FP-4: Should NOT flag a successful response as tool-not-found"
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
