#!/usr/bin/env bash
# ===========================================================================
# Integration Test: phase-proposal skill — P->E loop
# ===========================================================================
# Verifies AC-1: phase-proposal skill executes standalone, completes
# P->E (Planner -> Evaluator) loop, and reaches verdict=pass.
#
# Prerequisites:
#   - Claude Code CLI (`claude`) is on PATH
#   - OpenSpec change `<test-change>` does not already exist
#
# Usage:
#   ./test-phase-proposal.sh [test-change-name]
#   Default test-change-name: "test-phase-proposal-e2e"
#
# Steps:
#   1. Run `/dev-team:phase-proposal <test-change>`
#   2. Wait for completion
#   3. Check eval.json exists in openspec/changes/<test-change>/
#   4. Verify eval.json contains at least one entry for phase 01-proposal
#   5. Check verdict is "pass" (P->E loop completed successfully)
#   6. Report pass/fail
#   7. Clean up test change directory
#
# Expected outcome:
#   - eval.json written with phase 01-proposal entry
#   - verdict = "pass"
# ===========================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CHANGE_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"  # project root via ../.. from openspec/changes/<name>/
TEST_CHANGE="${1:-test-phase-proposal-e2e}"
TARGET_DIR="$CHANGE_DIR/openspec/changes/$TEST_CHANGE"

echo "============================================"
echo " Integration Test: phase-proposal P->E loop"
echo "============================================"
echo "Test change: $TEST_CHANGE"
echo "Target dir:  $TARGET_DIR"
echo ""

# ---- Step 0: Clean up any previous test run ----
if [ -d "$TARGET_DIR" ]; then
  echo "[CLEANUP] Removing previous test change directory..."
  rm -rf "$TARGET_DIR"
fi

# ---- Step 1: Run phase-proposal skill ----
echo "[EXEC] Running: /dev-team:phase-proposal $TEST_CHANGE"
echo ""

# TODO: Replace with actual Claude Code invocation
# claude -p "/dev-team:phase-proposal $TEST_CHANGE" -w "$CHANGE_DIR"
echo "[SKIP] Claude Code invocation skipped in skeleton — uncomment above line to run"

# ---- Step 2: Verify eval.json exists ----
echo ""
echo "[CHECK] Verifying eval.json exists..."

# TODO: Uncomment when running actual test
# if [ ! -f "$TARGET_DIR/eval.json" ]; then
#   echo "[FAIL] eval.json not found at $TARGET_DIR/eval.json"
#   exit 1
# fi
# echo "[PASS] eval.json exists"

# ---- Step 3: Verify phase 01-proposal entry ----
echo "[CHECK] Verifying eval.json contains 01-proposal entry..."

# TODO: Uncomment when running actual test
# ENTRIES=$(jq '[.[] | select(.phase == "01-proposal")] | length' "$TARGET_DIR/eval.json")
# if [ "$ENTRIES" -eq 0 ]; then
#   echo "[FAIL] No entries found for phase 01-proposal"
#   exit 1
# fi
# echo "[PASS] Found $ENTRIES entry/entries for phase 01-proposal"

# ---- Step 4: Verify verdict is "pass" ----
echo "[CHECK] Verifying verdict is 'pass'..."

# TODO: Uncomment when running actual test
# LATEST_ENTRY=$(jq '[.[] | select(.phase == "01-proposal")] | last' "$TARGET_DIR/eval.json")
# VERDICT=$(echo "$LATEST_ENTRY" | jq -r '.verdict')
# if [ "$VERDICT" != "pass" ]; then
#   echo "[FAIL] Verdict is '$VERDICT', expected 'pass'"
#   exit 1
# fi
# echo "[PASS] Verdict is 'pass'"

# ---- Step 5: Verify proposal.md and specs/ exist ----
echo "[CHECK] Verifying proposal.md and specs/ exist..."

# TODO: Uncomment when running actual test
# if [ ! -f "$TARGET_DIR/proposal.md" ]; then
#   echo "[FAIL] proposal.md not found"
#   exit 1
# fi
# echo "[PASS] proposal.md exists"

# if [ ! -d "$TARGET_DIR/specs" ] || [ -z "$(ls -A "$TARGET_DIR/specs/" 2>/dev/null)" ]; then
#   echo "[FAIL] specs/ directory is missing or empty"
#   exit 1
# fi
# echo "[PASS] specs/ directory exists with $(ls "$TARGET_DIR/specs/" | wc -l) capability(s)"

# ---- Step 6: Report overall result ----
echo ""
echo "============================================"
echo " Test Result: SKELETON (not executed)"
echo "============================================"
echo "To run this test:"
echo "  1. Uncomment the claude invocation and all [CHECK] blocks"
echo "  2. Ensure claude CLI is available"
echo "  3. Run: bash $0"
echo ""

# ---- Step 7: Clean up ----
echo "[CLEANUP] Removing test change directory..."
rm -rf "$TARGET_DIR"
echo "[CLEANUP] Done"
