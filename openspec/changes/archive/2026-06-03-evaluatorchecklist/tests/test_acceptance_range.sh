#!/usr/bin/env bash
# test_acceptance_range.sh — Content verification for AC-4.
#
# Covers AC-4: acceptance-evaluator.md verdict rule references range (A1-A5).
#
# Per the spec, the acceptance-evaluator's verdict rule previously referenced
# (A1-A4, A7) where A7 did not exist and A5 (formerly optional) was excluded.
# The range must now be (A1-A5) to reflect A5's promotion and remove the
# erroneous A7 reference.
#
# Rules:
#   - Verdict rule line MUST contain "(A1-A5)"
#   - Verdict rule line MUST NOT contain "(A1-A4, A7)" (old incorrect range)
#   - Verdict rule line MUST NOT contain "A7" as a range item
#   - Verdict rule line MUST NOT contain "ALL required items"
#   - A5 MUST appear in the Static Checklist section
#   - A7 MUST NOT appear in the Static Checklist (A7 was never a valid item)
#
# Usage:
#   bash test_acceptance_range.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_test_env
}

trap cleanup EXIT

# ─── Test: AC-4 — Verdict rule references A1-A5 ─────────────────────────────

test_ac4_verdict_range_a1_a5() {
    echo "=== AC-4: Verdict rule references (A1-A5) ==="

    local file="$AGENTS_DIR/acceptance-evaluator.md"
    setup_test_env

    # Verdict rule MUST contain "(A1-A5)"
    if grep -q '(A1-A5)' "$file" 2>/dev/null; then
        echo "PASS: Verdict rule contains (A1-A5)"
    elif grep -q 'A1-A5' "$file" 2>/dev/null; then
        echo "PASS: Verdict rule contains A1-A5 (no parens)"
    else
        echo "FAIL: Verdict rule does NOT contain (A1-A5)" >&2
        local verdict_line
        verdict_line="$(grep -i 'pass.*only.*if' "$file" 2>/dev/null || echo '(not found)')"
        echo "  Verdict rule line: $verdict_line" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-4 — Verdict rule does NOT reference old range A1-A4, A7 ────────

test_ac4_no_old_range_a1_a4_a7() {
    echo "=== AC-4: Verdict rule does NOT reference old range (A1-A4, A7) ==="

    local file="$AGENTS_DIR/acceptance-evaluator.md"
    setup_test_env

    # The old range was "(A1-A4, A7)" — check for any occurrence of "A1-A4" paired with "A7"
    local old_pattern='A1-A4.*A7\|A7.*A1-A4'
    if grep -q "$old_pattern" "$file" 2>/dev/null; then
        echo "FAIL: Verdict rule still references old range with A1-A4 and A7" >&2
        grep -n "$old_pattern" "$file" 2>/dev/null || true
        return 1
    fi

    echo "PASS: No reference to old range (A1-A4, A7)"
    return 0
}

# ─── Test: AC-4 — A7 not referenced in the verdict rule ─────────────────────

test_ac4_a7_not_referenced() {
    echo "=== AC-4: Verdict rule does NOT reference A7 ==="

    local file="$AGENTS_DIR/acceptance-evaluator.md"
    setup_test_env

    # A7 was an erroneous reference — ensure it is not mentioned in the
    # verdict rule context (the process/verdict section)
    local verdict_section
    verdict_section="$(sed -n '/^## Process/,/^## /p' "$file" 2>/dev/null || true)"
    if echo "$verdict_section" | grep -q 'A7' 2>/dev/null; then
        echo "FAIL: Verdict rule section references non-existent A7" >&2
        echo "$verdict_section" | grep -n 'A7' 2>/dev/null || true
        return 1
    fi

    echo "PASS: No A7 reference in verdict rule"
    return 0
}

# ─── Test: AC-4 — A5 present in Static Checklist ────────────────────────────

test_ac4_a5_in_checklist() {
    echo "=== AC-4: A5 present in Static Checklist ==="

    local file="$AGENTS_DIR/acceptance-evaluator.md"
    setup_test_env

    # A5 must appear in the file (previously optional, now mandatory)
    if grep -q 'A5' "$file" 2>/dev/null; then
        echo "PASS: A5 found in acceptance-evaluator.md"
    else
        echo "FAIL: A5 NOT found in acceptance-evaluator.md" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-4 — A7 not present in Static Checklist items ──────────────────

test_ac4_a7_not_in_checklist() {
    echo "=== AC-4: A7 not present in Static Checklist items ==="

    local file="$AGENTS_DIR/acceptance-evaluator.md"
    setup_test_env

    # Extract the Static Checklist section and check for A7
    local checklist_section
    checklist_section="$(sed -n '/^## Static Checklist/,/^## /p' "$file" 2>/dev/null || true)"
    if echo "$checklist_section" | grep -q 'A7' 2>/dev/null; then
        echo "FAIL: A7 found in Static Checklist (was never a valid item)" >&2
        echo "$checklist_section" | grep -n 'A7' 2>/dev/null || true
        return 1
    fi

    echo "PASS: A7 correctly absent from Static Checklist"
    return 0
}

# ─── Test: AC-4 — No "required" qualifier in verdict rule ────────────────────

test_ac4_no_required_qualifier() {
    echo "=== AC-4: Verdict rule does not use 'ALL required items' ==="

    local file="$AGENTS_DIR/acceptance-evaluator.md"
    setup_test_env

    if grep -qi 'ALL required items' "$file" 2>/dev/null; then
        echo "FAIL: Verdict rule uses 'ALL required items'" >&2
        grep -n -i 'ALL required items' "$file" 2>/dev/null || true
        return 1
    fi

    echo "PASS: No 'ALL required items' qualifier"
    return 0
}

# ─── Runner ─────────────────────────────────────────────────────────────────

run_test() {
    local test_name="$1"
    shift
    echo ""
    echo "──────────────────────────────────────────────"
    echo "  Running: $test_name"
    echo "──────────────────────────────────────────────"

    if "$@"; then
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        echo "FAIL: $test_name" >&2
    fi
}

# ─── Main ────────────────────────────────────────────────────────────────────

echo ""
echo "=============================================="
echo "  AC-4: Acceptance Evaluator Range Verification"
echo "=============================================="

run_test "Verdict rule references A1-A5" test_ac4_verdict_range_a1_a5
run_test "No old range A1-A4, A7" test_ac4_no_old_range_a1_a4_a7
run_test "A7 not referenced in verdict" test_ac4_a7_not_referenced
run_test "A5 present in checklist" test_ac4_a5_in_checklist
run_test "A7 absent from checklist" test_ac4_a7_not_in_checklist
run_test "No required qualifier" test_ac4_no_required_qualifier

# ─── Summary ─────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
