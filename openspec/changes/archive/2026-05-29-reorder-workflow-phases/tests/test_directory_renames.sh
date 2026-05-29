#!/usr/bin/env bash
# test_directory_renames.sh — Integration tests for directory and file renames (AC-2, AC-3)
#
# Covers:
#   AC-2: skills/phase-dev-design/ directory exists, skills/phase-dev-proposal/ does not
#   AC-3: agents/dev-design-planner.md and dev-design-evaluator.md exist, old names absent
#   Boundary: both old and new directories/files simultaneously present (residue detection)
#
# Usage:
#   bash test_directory_renames.sh
#   (run from any directory; resolves project root from script location)

set -euo pipefail

# ─── Resolve project root ────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
echo "Project root: $PROJECT_ROOT"

# Skils and agents live under plugins/dev-team/
SKILLS_DIR="$PROJECT_ROOT/plugins/dev-team/skills"
AGENTS_DIR="$PROJECT_ROOT/plugins/dev-team/agents"

# ─── Test counters ────────────────────────────────────────────────────────────
PASS=0
FAIL=0

assert_pass() {
    local test_name="$1"
    local message="$2"
    PASS=$((PASS + 1))
    echo "  PASS [${test_name}]: ${message}"
}

assert_fail() {
    local test_name="$1"
    local message="$2"
    FAIL=$((FAIL + 1))
    echo "  FAIL [${test_name}]: ${message}" >&2
}

# =============================================================================
# AC-2: Skills directory rename
# =============================================================================

echo "=== AC-2: Skills directory rename ==="

# AC-2: New dev-design directory exists
if [[ -d "$SKILLS_DIR/phase-dev-design" ]]; then
    assert_pass "AC-2a" "plugins/dev-team/skills/phase-dev-design/ directory exists"
else
    assert_fail "AC-2a" "plugins/dev-team/skills/phase-dev-design/ directory does NOT exist"
fi

# AC-2: Old dev-proposal directory does NOT exist
if [[ ! -d "$SKILLS_DIR/phase-dev-proposal" ]]; then
    assert_pass "AC-2b" "plugins/dev-team/skills/phase-dev-proposal/ directory does NOT exist (correctly removed)"
else
    assert_fail "AC-2b" "plugins/dev-team/skills/phase-dev-proposal/ directory still EXISTS (should be removed or renamed)"
fi

# AC-2: SKILL.md exists in new location
if [[ -f "$SKILLS_DIR/phase-dev-design/SKILL.md" ]]; then
    assert_pass "AC-2c" "plugins/dev-team/skills/phase-dev-design/SKILL.md exists"
else
    assert_fail "AC-2c" "plugins/dev-team/skills/phase-dev-design/SKILL.md does NOT exist"
fi

# AC-2: Old SKILL.md does not exist in old location
if [[ ! -f "$SKILLS_DIR/phase-dev-proposal/SKILL.md" ]]; then
    assert_pass "AC-2d" "plugins/dev-team/skills/phase-dev-proposal/SKILL.md does NOT exist"
else
    assert_fail "AC-2d" "plugins/dev-team/skills/phase-dev-proposal/SKILL.md still EXISTS"
fi

# Boundary: Warn if both directories exist (rename residue)
if [[ -d "$SKILLS_DIR/phase-dev-design" && -d "$SKILLS_DIR/phase-dev-proposal" ]]; then
    assert_fail "AC-2-boundary" "BOTH phase-dev-design AND phase-dev-proposal directories exist — rename conflict detected, old directory should be removed"
else
    assert_pass "AC-2-boundary" "No directory rename conflict detected (only one version exists)"
fi

echo ""

# =============================================================================
# AC-3: Agent files rename
# =============================================================================

echo "=== AC-3: Agent files rename ==="

# AC-3a: dev-design-planner.md exists
if [[ -f "$AGENTS_DIR/dev-design-planner.md" ]]; then
    assert_pass "AC-3a" "plugins/dev-team/agents/dev-design-planner.md exists"
else
    assert_fail "AC-3a" "plugins/dev-team/agents/dev-design-planner.md does NOT exist"
fi

# AC-3b: dev-design-evaluator.md exists
if [[ -f "$AGENTS_DIR/dev-design-evaluator.md" ]]; then
    assert_pass "AC-3b" "plugins/dev-team/agents/dev-design-evaluator.md exists"
else
    assert_fail "AC-3b" "plugins/dev-team/agents/dev-design-evaluator.md does NOT exist"
fi

# AC-3c: Old dev-proposal-planner.md does NOT exist
if [[ ! -f "$AGENTS_DIR/dev-proposal-planner.md" ]]; then
    assert_pass "AC-3c" "plugins/dev-team/agents/dev-proposal-planner.md does NOT exist (correctly renamed)"
else
    assert_fail "AC-3c" "plugins/dev-team/agents/dev-proposal-planner.md still EXISTS (should be renamed)"
fi

# AC-3d: Old dev-proposal-evaluator.md does NOT exist
if [[ ! -f "$AGENTS_DIR/dev-proposal-evaluator.md" ]]; then
    assert_pass "AC-3d" "plugins/dev-team/agents/dev-proposal-evaluator.md does NOT exist (correctly renamed)"
else
    assert_fail "AC-3d" "plugins/dev-team/agents/dev-proposal-evaluator.md still EXISTS (should be renamed)"
fi

# Boundary: Both old and new agent files existing simultaneously
if [[ -f "$AGENTS_DIR/dev-design-planner.md" && -f "$AGENTS_DIR/dev-proposal-planner.md" ]]; then
    assert_fail "AC-3-boundary-planner" "BOTH dev-design-planner.md AND dev-proposal-planner.md exist — rename residue"
else
    assert_pass "AC-3-boundary-planner" "No planner rename residue"
fi

if [[ -f "$AGENTS_DIR/dev-design-evaluator.md" && -f "$AGENTS_DIR/dev-proposal-evaluator.md" ]]; then
    assert_fail "AC-3-boundary-evaluator" "BOTH dev-design-evaluator.md AND dev-proposal-evaluator.md exist — rename residue"
else
    assert_pass "AC-3-boundary-evaluator" "No evaluator rename residue"
fi

echo ""

# =============================================================================
# Summary
# =============================================================================
echo "=== Summary ==="
echo "Total: $((PASS + FAIL)) | Passed: ${PASS} | Failed: ${FAIL}"

if [[ "$FAIL" -gt 0 ]]; then
    exit 1
fi
exit 0
