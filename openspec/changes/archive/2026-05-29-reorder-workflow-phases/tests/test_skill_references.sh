#!/usr/bin/env bash
# test_skill_references.sh — Integration tests for SKILL.md phase references (AC-6, AC-7)
#
# Covers:
#   AC-6: phase-dev-design/SKILL.md has name "phase-dev-design", gate check uses 02-dev-design
#   AC-7: phase-test-design/SKILL.md gate check uses 03-test-design (not 02-test-design)
#
# Usage:
#   bash test_skill_references.sh
#   (run from any directory; resolves project root from script location)

set -euo pipefail

# ─── Resolve project root ────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
echo "Project root: $PROJECT_ROOT"

# Skill files live under plugins/dev-team/
SKILLS_DIR="$PROJECT_ROOT/plugins/dev-team/skills"

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
# AC-6: phase-dev-design/SKILL.md references
# =============================================================================

echo "=== AC-6: phase-dev-design/SKILL.md references ==="

SKILL_FILE="$SKILLS_DIR/phase-dev-design/SKILL.md"

if [[ ! -f "$SKILL_FILE" ]]; then
    assert_fail "AC-6-file" "plugins/dev-team/skills/phase-dev-design/SKILL.md not found"
else
    SKILL_CONTENT=$(cat "$SKILL_FILE")

    # AC-6: name should be phase-dev-design
    if echo "$SKILL_CONTENT" | grep -qiE "name.*phase-dev-design"; then
        assert_pass "AC-6a" "SKILL.md name is phase-dev-design"
    else
        assert_fail "AC-6a" "SKILL.md name is NOT phase-dev-design"
    fi

    # AC-6: name should NOT be phase-dev-proposal (old name)
    if echo "$SKILL_CONTENT" | grep -qi "phase-dev-proposal"; then
        assert_fail "AC-6b" "SKILL.md still contains old name 'phase-dev-proposal'"
    else
        assert_pass "AC-6b" "SKILL.md does NOT contain old name 'phase-dev-proposal'"
    fi

    # AC-6: gate check phase should be 02-dev-design
    if echo "$SKILL_CONTENT" | grep -qiE "eval_check.*02-dev-design"; then
        assert_pass "AC-6c" "SKILL.md gate check uses --phase 02-dev-design"
    else
        assert_fail "AC-6c" "SKILL.md gate check does NOT use --phase 02-dev-design"
    fi

    # AC-6: gate check should NOT use old 03-dev-proposal
    if echo "$SKILL_CONTENT" | grep -qiE "eval_check.*03-dev-proposal"; then
        assert_fail "AC-6d" "SKILL.md gate check still uses old --phase 03-dev-proposal"
    else
        assert_pass "AC-6d" "SKILL.md gate check does NOT use old --phase 03-dev-proposal"
    fi

    # AC-6: Should reference dev-design-planner (not dev-proposal-planner)
    if echo "$SKILL_CONTENT" | grep -qiE "dev-design-planner"; then
        assert_pass "AC-6e" "SKILL.md references dev-design-planner agent"
    else
        assert_fail "AC-6e" "SKILL.md does NOT reference dev-design-planner agent"
    fi

    # AC-6: Should reference dev-design-evaluator (not dev-proposal-evaluator)
    if echo "$SKILL_CONTENT" | grep -qiE "dev-design-evaluator"; then
        assert_pass "AC-6f" "SKILL.md references dev-design-evaluator agent"
    else
        assert_fail "AC-6f" "SKILL.md does NOT reference dev-design-evaluator agent"
    fi

    # AC-6: Should NOT reference dev-proposal-planner
    if echo "$SKILL_CONTENT" | grep -qiE "dev-proposal-planner"; then
        assert_fail "AC-6g" "SKILL.md still references old dev-proposal-planner agent"
    else
        assert_pass "AC-6g" "SKILL.md does NOT reference old dev-proposal-planner agent"
    fi

    # AC-6: Should NOT reference dev-proposal-evaluator
    if echo "$SKILL_CONTENT" | grep -qiE "dev-proposal-evaluator"; then
        assert_fail "AC-6h" "SKILL.md still references old dev-proposal-evaluator agent"
    else
        assert_pass "AC-6h" "SKILL.md does NOT reference old dev-proposal-evaluator agent"
    fi
fi

echo ""

# =============================================================================
# AC-7: phase-test-design/SKILL.md phase reference
# =============================================================================

echo "=== AC-7: phase-test-design/SKILL.md gate check ==="

TEST_SKILL_FILE="$SKILLS_DIR/phase-test-design/SKILL.md"

if [[ ! -f "$TEST_SKILL_FILE" ]]; then
    assert_fail "AC-7-file" "plugins/dev-team/skills/phase-test-design/SKILL.md not found"
else
    TEST_SKILL_CONTENT=$(cat "$TEST_SKILL_FILE")

    # AC-7: gate check phase should be 03-test-design
    if echo "$TEST_SKILL_CONTENT" | grep -qiE "eval_check.*03-test-design"; then
        assert_pass "AC-7a" "phase-test-design SKILL.md gate check uses --phase 03-test-design"
    else
        assert_fail "AC-7a" "phase-test-design SKILL.md gate check does NOT use --phase 03-test-design"
    fi

    # AC-7: gate check should NOT use old 02-test-design
    if echo "$TEST_SKILL_CONTENT" | grep -qiE "eval_check.*02-test-design"; then
        assert_fail "AC-7b" "phase-test-design SKILL.md gate check still uses old --phase 02-test-design"
    else
        assert_pass "AC-7b" "phase-test-design SKILL.md gate check does NOT use old --phase 02-test-design"
    fi

    # AC-7: Should not contain old phase-dev-proposal references
    if echo "$TEST_SKILL_CONTENT" | grep -qiE "dev-proposal"; then
        assert_fail "AC-7c" "phase-test-design SKILL.md still references 'dev-proposal'"
    else
        assert_pass "AC-7c" "phase-test-design SKILL.md does NOT reference 'dev-proposal'"
    fi
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
