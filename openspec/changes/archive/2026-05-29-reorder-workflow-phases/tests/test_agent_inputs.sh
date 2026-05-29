#!/usr/bin/env bash
# test_agent_inputs.sh — Integration tests for agent Input section content (AC-4, AC-5)
#
# Covers:
#   AC-4: dev-design-planner.md Input section lists proposal.md and codebase, NOT test-design.md
#   AC-5: test-design-planner.md Input section lists proposal.md AND design.md
#
# Usage:
#   bash test_agent_inputs.sh
#   (run from any directory; resolves project root from script location)

set -euo pipefail

# ─── Resolve project root ────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
echo "Project root: $PROJECT_ROOT"

# Agent files live under plugins/dev-team/
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

# ─── Helper: extract Input section from agent.md ──────────────────────────────
extract_input_section() {
    local agent_file="$1"
    # Extract lines from "## Input" to the next "## " heading (not ## in code fences)
    awk '/^## Input$/ {flag=1; next} /^## / && flag {flag=0} flag' "$agent_file"
}

# =============================================================================
# AC-4: dev-design-planner inputs (proposal.md + codebase, NOT test-design.md)
# =============================================================================

echo "=== AC-4: dev-design-planner Input section ==="

DEV_DESIGN_PLANNER="$AGENTS_DIR/dev-design-planner.md"

if [[ ! -f "$DEV_DESIGN_PLANNER" ]]; then
    assert_fail "AC-4a" "plugins/dev-team/agents/dev-design-planner.md not found — cannot verify inputs"
else
    INPUT_TEXT=$(extract_input_section "$DEV_DESIGN_PLANNER")

    # AC-4: Should reference proposal.md
    if echo "$INPUT_TEXT" | grep -qi "proposal\.md"; then
        assert_pass "AC-4a" "dev-design-planner Input section references proposal.md"
    else
        assert_fail "AC-4a" "dev-design-planner Input section does NOT reference proposal.md"
    fi

    # AC-4: Should reference codebase (or CLAUDE.md)
    if echo "$INPUT_TEXT" | grep -qiE "(codebase|CLAUDE\.md|project.*code|source.*code)"; then
        assert_pass "AC-4b" "dev-design-planner Input section references codebase context"
    else
        assert_fail "AC-4b" "dev-design-planner Input section does NOT reference codebase context"
    fi

    # AC-4: Should NOT reference test-design.md
    if echo "$INPUT_TEXT" | grep -qi "test-design\.md"; then
        assert_fail "AC-4c" "dev-design-planner Input section still references test-design.md (should NOT — dev-design runs before test-design)"
    else
        assert_pass "AC-4c" "dev-design-planner Input section does NOT reference test-design.md"
    fi
fi

echo ""

# =============================================================================
# AC-5: test-design-planner inputs (proposal.md + design.md)
# =============================================================================

echo "=== AC-5: test-design-planner Input section ==="

TEST_DESIGN_PLANNER="$AGENTS_DIR/test-design-planner.md"

if [[ ! -f "$TEST_DESIGN_PLANNER" ]]; then
    assert_fail "AC-5a" "plugins/dev-team/agents/test-design-planner.md not found — cannot verify inputs"
else
    INPUT_TEXT=$(extract_input_section "$TEST_DESIGN_PLANNER")

    # AC-5: Should reference proposal.md
    if echo "$INPUT_TEXT" | grep -qi "proposal\.md"; then
        assert_pass "AC-5a" "test-design-planner Input section references proposal.md"
    else
        assert_fail "AC-5a" "test-design-planner Input section does NOT reference proposal.md"
    fi

    # AC-5: Should reference design.md (new input for test-design after reorder)
    if echo "$INPUT_TEXT" | grep -qi "design\.md"; then
        assert_pass "AC-5b" "test-design-planner Input section references design.md (architecture context from 02-dev-design)"
    else
        assert_fail "AC-5b" "test-design-planner Input section does NOT reference design.md"
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
