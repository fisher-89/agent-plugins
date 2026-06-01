#!/usr/bin/env bash
# ===========================================================================
# Integration Test: workflow-requirement — full 9-phase workflow
# ===========================================================================
# Verifies AC-4: workflow-requirement skill automatically executes
# Phase 01-09 in sequence and completes all phases.
#
# Prerequisites:
#   - Claude Code CLI (`claude`) is on PATH
#   - OpenSpec change `<test-change>` does not already exist
#   - Test may take 10-30 minutes to complete
#
# Usage:
#   ./test-workflow-full.sh [test-change-name]
#   Default test-change-name: "test-workflow-full-e2e"
#
# Checks:
#   1. Workflow runs without error
#   2. eval.json contains entries for all 9 phases
#   3. All 9 phases have verdict "pass" (or "pass" with skipped)
#   4. Completion notification is displayed
#   5. Archive is NOT automatically executed
# ===========================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CHANGE_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
TEST_CHANGE="${1:-test-workflow-full-e2e}"
TARGET_DIR="$CHANGE_DIR/openspec/changes/$TEST_CHANGE"

echo "============================================"
echo " Integration Test: workflow-requirement full"
echo "============================================"
echo "Test change: $TEST_CHANGE"
echo "Target dir:  $TARGET_DIR"
echo ""

# ---- Step 0: Clean up ----
if [ -d "$TARGET_DIR" ]; then
  echo "[CLEANUP] Removing previous test change directory..."
  rm -rf "$TARGET_DIR"
fi

# ---- Step 1: Run workflow-requirement ----
echo "[EXEC] Running: /dev-team:workflow-requirement $TEST_CHANGE"
echo "[NOTE] This test may take 10-30 minutes to complete"
echo ""

# TODO: Replace with actual Claude Code invocation
# claude -p "/dev-team:workflow-requirement $TEST_CHANGE" -w "$CHANGE_DIR"
echo "[SKIP] Claude Code invocation skipped in skeleton"

# ---- Step 2: Verify eval.json exists ----
echo "[CHECK] Verifying eval.json exists..."

# TODO: Uncomment when running actual test
# if [ ! -f "$TARGET_DIR/eval.json" ]; then
#   echo "[FAIL] eval.json not found at $TARGET_DIR/eval.json"
#   exit 1
# fi
# echo "[PASS] eval.json exists"

# ---- Step 3: Verify all 9 phases have entries ----
echo "[CHECK] Verifying all 9 phases have entries..."

# TODO: Uncomment when running actual test
# EXPECTED_PHASES=(
#   "01-proposal"
#   "02-dev-design"
#   "03-test-design"
#   "04-test-gen"
#   "05-implement"
#   "06-unit-test"
#   "07-code-review"
#   "08-integration-test"
#   "09-acceptance"
# )
#
# for phase in "${EXPECTED_PHASES[@]}"; do
#   COUNT=$(jq "[.[] | select(.phase == \"$phase\")] | length" "$TARGET_DIR/eval.json")
#   if [ "$COUNT" -eq 0 ]; then
#     echo "[FAIL] No entries found for phase $phase"
#     exit 1
#   fi
#   echo "  [PASS] Phase $phase has $COUNT entry/entries"
# done

# ---- Step 4: Verify all phases have pass verdict ----
echo "[CHECK] Verifying all phases have pass verdict..."

# TODO: Uncomment when running actual test
# for phase in "${EXPECTED_PHASES[@]}"; do
#   LATEST=$(jq "[.[] | select(.phase == \"$phase\")] | last" "$TARGET_DIR/eval.json")
#   VERDICT=$(echo "$LATEST" | jq -r '.verdict')
#   if [ "$VERDICT" != "pass" ]; then
#     echo "[FAIL] Phase $phase latest verdict is '$VERDICT', expected 'pass'"
#     exit 1
#   fi
#   echo "  [PASS] Phase $phase verdict: $VERDICT"
# done

# ---- Step 5: Verify phase order (check attempt timestamps advance) ----
echo "[CHECK] Verifying phase execution order..."

# TODO: Uncomment when running actual test
# PREV_TS=""
# for phase in "${EXPECTED_PHASES[@]}"; do
#   LATEST=$(jq "[.[] | select(.phase == \"$phase\")] | last" "$TARGET_DIR/eval.json")
#   TS=$(echo "$LATEST" | jq -r '.timestamp')
#   if [ -n "$PREV_TS" ] && [ "$(date -d "$TS" +%s)" -lt "$(date -d "$PREV_TS" +%s)" ]; then
#     echo "[WARN] Phase $phase has timestamp earlier than previous phase"
#   fi
#   PREV_TS="$TS"
# done
# echo "[PASS] Phase order verified"

# ---- Step 6: Verify archive was NOT called ----
echo "[CHECK] Verifying archive was NOT automatically executed..."

# TODO: Uncomment when running actual test
# ARCHIVE_DIR="$CHANGE_DIR/openspec/changes/archive"
# if [ -d "$ARCHIVE_DIR/$TEST_CHANGE" ]; then
#   echo "[FAIL] Change was automatically archived — expected manual archive only"
#   exit 1
# fi
# echo "[PASS] No automatic archive detected (manual archive is correct)"

# ---- Report ----
echo ""
echo "============================================"
echo " Test Result: SKELETON (not executed)"
echo "============================================"
echo "To run this test:"
echo "  1. Uncomment the claude invocation and all [CHECK] blocks"
echo "  2. Ensure claude CLI is available"
echo "  3. Run: bash $0"
echo ""

# ---- Clean up ----
echo "[CLEANUP] Removing test change directory..."
rm -rf "$TARGET_DIR"
echo "[CLEANUP] Done"
