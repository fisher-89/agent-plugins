#!/usr/bin/env bash
# ===========================================================================
# Integration Test: proposal-planner output — artifact completeness
# ===========================================================================
# Verifies AC-2: proposal-planner agent correctly writes proposal.md + specs/
# with complete file structure and no placeholder content.
#
# Prerequisites:
#   - Claude Code CLI (`claude`) is on PATH
#   - OpenSpec change `<test-change>` does not already exist
#
# Usage:
#   ./test-planner-output.sh [test-change-name]
#   Default test-change-name: "test-planner-output-check"
#
# Checks:
#   1. proposal.md exists and contains all required sections
#   2. specs/ directory exists
#   3. Each spec.md has ADDED/MODIFIED delta headers
#   4. No placeholder content (TODO, TBD, {{...}}) remains
#   5. spec files use correct WHEN/THEN scenario format
# ===========================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CHANGE_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
TEST_CHANGE="${1:-test-planner-output-check}"
TARGET_DIR="$CHANGE_DIR/openspec/changes/$TEST_CHANGE"

echo "============================================"
echo " Integration Test: proposal-planner output"
echo "============================================"
echo "Test change: $TEST_CHANGE"
echo "Target dir:  $TARGET_DIR"
echo ""

# ---- Step 0: Clean up ----
if [ -d "$TARGET_DIR" ]; then
  echo "[CLEANUP] Removing previous test change directory..."
  rm -rf "$TARGET_DIR"
fi

# ---- Step 1: Run phase-proposal to generate artifacts ----
echo "[EXEC] Running: /dev-team:phase-proposal $TEST_CHANGE"
echo ""

# TODO: Replace with actual Claude Code invocation
# claude -p "/dev-team:phase-proposal $TEST_CHANGE" -w "$CHANGE_DIR"
echo "[SKIP] Claude Code invocation skipped in skeleton"

# ---- Step 2: Verify proposal.md structure ----
echo "[CHECK] Verifying proposal.md sections..."

# TODO: Uncomment when running actual test
# REQUIRED_SECTIONS=("## 问题" "## 提案" "## 能力" "## 变更范围" "## 验收标准" "## 风险")
# for section in "${REQUIRED_SECTIONS[@]}"; do
#   if ! grep -q "$section" "$TARGET_DIR/proposal.md"; then
#     echo "[FAIL] Missing section: $section in proposal.md"
#     exit 1
#   fi
# done
# echo "[PASS] All required sections present in proposal.md"

# ---- Step 3: Check for placeholder content ----
echo "[CHECK] Checking for placeholder content..."

# TODO: Uncomment when running actual test
# if grep -qiE '\bTODO\b|TBD|INSERT|{{[^}]+}}' "$TARGET_DIR/proposal.md"; then
#   echo "[FAIL] Placeholder content found in proposal.md (TODO/TBD/{{...}})"
#   exit 1
# fi
# echo "[PASS] No placeholder content in proposal.md"

# ---- Step 4: Verify specs/ directory ----
echo "[CHECK] Verifying specs/ directory..."

# TODO: Uncomment when running actual test
# if [ ! -d "$TARGET_DIR/specs" ]; then
#   echo "[FAIL] specs/ directory not found"
#   exit 1
# fi
#
# SPEC_COUNT=$(find "$TARGET_DIR/specs" -name "spec.md" | wc -l)
# if [ "$SPEC_COUNT" -eq 0 ]; then
#   echo "[FAIL] No spec.md files found in specs/"
#   exit 1
# fi
# echo "[PASS] Found $SPEC_COUNT spec.md file(s)"

# ---- Step 5: Verify spec.md delta headers ----
echo "[CHECK] Verifying spec.md delta headers..."

# TODO: Uncomment when running actual test
# for spec in "$TARGET_DIR/specs"/*/spec.md; do
#   if [ ! -f "$spec" ]; then
#     continue
#   fi
#   if ! grep -qE '^## (ADDED|MODIFIED|REMOVED|RENAMED) Requirements' "$spec"; then
#     echo "[FAIL] $spec missing delta header (## ADDED/MODIFIED/REMOVED/RENAMED Requirements)"
#     exit 1
#   fi
# done
# echo "[PASS] All spec files have valid delta headers"

# ---- Step 6: Verify WHEN/THEN scenarios in specs ----
echo "[CHECK] Verifying WHEN/THEN scenarios..."

# TODO: Uncomment when running actual test
# for spec in "$TARGET_DIR/specs"/*/spec.md; do
#   if [ ! -f "$spec" ]; then
#     continue
#   fi
#   SCENARIO_COUNT=$(grep -c '#### Scenario:' "$spec" || true)
#   if [ "$SCENARIO_COUNT" -eq 0 ]; then
#     echo "[FAIL] $spec has no scenarios (#### Scenario:)"
#     exit 1
#   fi
#   if ! grep -q '\*\*WHEN\*\*' "$spec" || ! grep -q '\*\*THEN\*\*' "$spec"; then
#     echo "[FAIL] $spec missing WHEN/THEN format in scenarios"
#     exit 1
#   fi
# done
# echo "[PASS] All spec files contain WHEN/THEN scenarios"

# ---- Step 7: Check placeholder in specs ----
echo "[CHECK] No placeholder content in specs..."

# TODO: Uncomment when running actual test
# for spec in "$TARGET_DIR/specs"/*/spec.md; do
#   if [ -f "$spec" ] && grep -qiE '\bTODO\b|TBD|{{[^}]+}}' "$spec"; then
#     echo "[FAIL] Placeholder content found in $spec"
#     exit 1
#   fi
# done
# echo "[PASS] No placeholder content in specs"

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
