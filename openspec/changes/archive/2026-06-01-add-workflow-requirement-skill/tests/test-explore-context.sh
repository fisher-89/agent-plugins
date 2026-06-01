#!/usr/bin/env bash
# ===========================================================================
# Integration Test: explore context inheritance — AC-5
# ===========================================================================
# Verifies AC-5: workflow-requirement detects explore context from the
# conversation and passes EXPLORE_CONTEXT_SUMMARY to proposal-planner.
#
# Prerequisites:
#   - Claude Code CLI (`claude`) is on PATH
#   - Requires an explore session context in the conversation
#
# Usage:
#   ./test-explore-context.sh [test-change-name]
#   Default test-change-name: "test-explore-context-e2e"
#
# NOTE: This test requires an explore session to have been run prior to
# invoking the workflow. The explore context detection relies on conversation
# history. Run this test after an explore session.
#
# Steps:
#   1. Run explore session (generate context)
#   2. Run `/dev-team:workflow-requirement` (no change name — auto-detect)
#   3. Check that workflow detects explore context
#   4. Check that proposal-planner receives explore context in prompt
#   5. Check that proposal includes explore-derived content
#   6. Report pass/fail
#
# Alternative: Verify via eval/next input that explore_context is accepted
# and passed to planner prompt.
# ===========================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CHANGE_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
TEST_CHANGE="${1:-test-explore-context-e2e}"
TARGET_DIR="$CHANGE_DIR/openspec/changes/$TEST_CHANGE"

echo "============================================"
echo " Integration Test: explore context inheritance"
echo "============================================"
echo "Test change: $TEST_CHANGE"
echo ""

# ---- Step 0: Clean up ----
if [ -d "$TARGET_DIR" ]; then
  echo "[CLEANUP] Removing previous test change directory..."
  rm -rf "$TARGET_DIR"
fi

# ---- Step 1: Run explore session (if not already done) ----
echo "[NOTE] This test requires prior explore context in conversation history."
echo "[NOTE] If no explore context exists, run an explore session first:"
echo "        /dev-team:openspec-explore <topic>"
echo ""

# ---- Step 2: Run workflow-requirement without change name ----
echo "[EXEC] Running: /dev-team:workflow-requirement (no change name)"
echo ""

# TODO: Replace with actual Claude Code invocation
# claude -p "/dev-team:workflow-requirement" -w "$CHANGE_DIR"
echo "[SKIP] Claude Code invocation skipped in skeleton"
echo "[SKIP] This test requires interactive explore context — cannot fully automate"

# ---- Step 3: Verify change was created with derived name ----
echo "[CHECK] Verifying change was created..."

# TODO: Uncomment when running actual test
# if [ ! -d "$TARGET_DIR" ]; then
#   echo "[FAIL] Change directory was not created"
#   echo "[HINT] The workflow should have derived a change name from explore context"
#   echo "  Expected dir: $TARGET_DIR"
#   exit 1
# fi
# echo "[PASS] Change directory created: $(basename "$TARGET_DIR")"

# ---- Step 4: Verify proposal.md references explore context ----
echo "[CHECK] Checking proposal.md for explore-derived content..."

# TODO: Uncomment when running actual test
# if [ ! -f "$TARGET_DIR/proposal.md" ]; then
#   echo "[FAIL] proposal.md not found"
#   exit 1
# fi
#
# # Look for explore-derived keywords
# if grep -qiE '探索|explore|决策|分析|对比|diagram|架构' "$TARGET_DIR/proposal.md"; then
#   echo "[PASS] proposal.md contains explore-derived content"
# else
#   echo "[WARN] proposal.md does not contain obvious explore-derived keywords"
#   echo "[HINT] Check manually: $TARGET_DIR/proposal.md"
# fi

# ---- Step 5: Verify specs were created ----
echo "[CHECK] Verifying specs were created..."

# TODO: Uncomment when running actual test
# if [ -d "$TARGET_DIR/specs" ] && [ "$(find "$TARGET_DIR/specs" -name "spec.md" 2>/dev/null | wc -l)" -gt 0 ]; then
#   echo "[PASS] Specs created ($(find "$TARGET_DIR/specs" -name "spec.md" | wc -l) spec files)"
# else
#   echo "[FAIL] No specs created"
#   exit 1
# fi

# ---- Step 6: Verify via eval/next that explore_context is accepted ----
echo "[CHECK] Verifying eval/next accepts explore_context parameter..."

# TODO: Uncomment when running actual test
# This requires running an MCP tool call to eval/next with explore_context
# and checking the planner prompt contains the context.
# Example check via MCP server:
#   result=$(claude mcp eval/next --change "$TEST_CHANGE" \
#     --explore_context "Summary of explore session: ...")
#   echo "$result" | jq -r '.planner.prompt' | grep -q "探索上下文" || true
# echo "[SKIP] eval/next explore_context parameter check requires MCP server"

# ---- Report ----
echo ""
echo "============================================"
echo " Test Result: SKELETON (not executed)"
echo "============================================"
echo "To run this test:"
echo "  1. Run an explore session first: /dev-team:openspec-explore <topic>"
echo "  2. Uncomment the claude invocation and [CHECK] blocks"
echo "  3. Run: bash $0"
echo ""

# ---- Clean up ----
echo "[CLEANUP] Removing test change directory..."
rm -rf "$TARGET_DIR"
echo "[CLEANUP] Done"
