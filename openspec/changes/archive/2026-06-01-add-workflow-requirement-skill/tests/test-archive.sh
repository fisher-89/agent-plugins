#!/usr/bin/env bash
# ===========================================================================
# Integration Test: workflow completion — no automatic archive — AC-9
# ===========================================================================
# Verifies AC-9: After all phases pass, workflow-requirement stops and
# does NOT automatically archive the change. User must manually run
# `/dev-team:openspec-archive-change`.
#
# Prerequisites:
#   - A completed workflow run exists (eval.json with all 9 phases passed)
#   - Claude Code CLI (`claude`) is on PATH
#
# Usage:
#   ./test-archive.sh <existing-change-name>
#
# Steps:
#   1. Confirm the change directory exists with complete eval.json
#   2. Verify the change is NOT in openspec/changes/archive/
#   3. Verify workflow completion output includes manual archive reminder
#   4. Re-run workflow to confirm it returns done immediately (no re-execution)
#   5. Report pass/fail
# ===========================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CHANGE_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"

if [ $# -lt 1 ]; then
  echo "Usage: $0 <existing-change-name>"
  echo "Example: $0 my-completed-change"
  echo ""
  echo "Prerequisites:"
  echo "  - The change must have all 9 phases with verdict=pass in eval.json"
  exit 1
fi

TEST_CHANGE="$1"
TARGET_DIR="$CHANGE_DIR/openspec/changes/$TEST_CHANGE"
ARCHIVE_DIR="$CHANGE_DIR/openspec/changes/archive"

echo "============================================"
echo " Integration Test: no automatic archive (AC-9)"
echo "============================================"
echo "Test change: $TEST_CHANGE"
echo "Target dir:  $TARGET_DIR"
echo ""

# ---- Step 1: Verify change directory exists ----
echo "[CHECK] Verifying change directory exists..."

# TODO: Uncomment when running actual test
# if [ ! -d "$TARGET_DIR" ]; then
#   echo "[FAIL] Change directory does not exist: $TARGET_DIR"
#   exit 1
# fi
# echo "[PASS] Change directory exists"

# ---- Step 2: Verify eval.json exists and has all 9 phases ----
echo "[CHECK] Verifying eval.json has all 9 phases..."

# TODO: Uncomment when running actual test
# if [ ! -f "$TARGET_DIR/eval.json" ]; then
#   echo "[FAIL] eval.json not found"
#   exit 1
# fi
#
# PHASE_COUNT=$(jq '[.[] | select(.verdict == "pass") | .phase] | unique | length' "$TARGET_DIR/eval.json")
# if [ "$PHASE_COUNT" -lt 9 ]; then
#   echo "[FAIL] Only $PHASE_COUNT phases have pass verdict (expected 9)"
#   echo "[HINT] Complete all phases first by running /dev-team:workflow-requirement $TEST_CHANGE"
#   exit 1
# fi
# echo "[PASS] All 9 phases have pass verdict"

# ---- Step 3: Verify change is NOT archived ----
echo "[CHECK] Verifying change was NOT automatically archived..."

# TODO: Uncomment when running actual test
# if [ -d "$ARCHIVE_DIR/$TEST_CHANGE" ]; then
#   echo "[FAIL] Change was already archived at $ARCHIVE_DIR/$TEST_CHANGE"
#   echo "[HINT] This test expects the change to be in openspec/changes/, not archive/"
#   exit 1
# fi
# echo "[PASS] Change is NOT in archive (correct behavior: manual archive required)"

# ---- Step 4: Verify workflow reports done on re-run ----
echo "[CHECK] Verifying workflow returns done immediately on re-run..."
echo "[NOTE] Re-running should detect all phases passed and return without re-executing"

# TODO: Uncomment when running actual test
# claude -p "/dev-team:workflow-requirement $TEST_CHANGE" -w "$CHANGE_DIR" | tee /tmp/workflow-archive-test.log
# if grep -qi "已完成|done|complete|全部 phase" /tmp/workflow-archive-test.log; then
#   echo "[PASS] Workflow reports completion without re-execution"
# else
#   echo "[WARN] Could not detect completion summary in output"
#   echo "[HINT] Check /tmp/workflow-archive-test.log for workflow output"
# fi
#
# if grep -qi "手动.*archive\|archive.*手动\|openspec-archive" /tmp/workflow-archive-test.log; then
#   echo "[PASS] Workflow output includes manual archive reminder"
# else
#   echo "[WARN] Could not detect manual archive reminder in output"
# fi

# ---- Step 5: Verify manual archive works ----
echo "[CHECK] Verifying manual archive works via /dev-team:openspec-archive-change..."

# TODO: Uncomment when running actual test
# echo "[NOTE] Archive test is OPTIONAL — run manually:"
# echo "  /dev-team:openspec-archive-change $TEST_CHANGE"
# echo ""
# echo "[SKIP] Manual archive step skipped in skeleton test"

# ---- Report ----
echo ""
echo "============================================"
echo " Test Result: SKELETON (not executed)"
echo "============================================"
echo "To run this test:"
echo "  1. Complete a full workflow: /dev-team:workflow-requirement <change>"
echo "  2. Uncomment all [CHECK] blocks in this script"
echo "  3. Run: bash $0 <change>"
echo "  4. Verify change is NOT automatically archived"
echo ""
