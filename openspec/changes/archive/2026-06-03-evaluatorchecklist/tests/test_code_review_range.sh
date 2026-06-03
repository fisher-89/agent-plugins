#!/usr/bin/env bash
# test_code_review_range.sh — Content verification for AC-3.
#
# Covers AC-3: code-review-evaluator.md verdict rule references range (C1-C8).
#
# Per the spec, the code-review-evaluator's verdict rule previously referenced
# (C1-C5), which was incomplete. It must now reference (C1-C8) to cover all
# items in the Static Checklist.
#
# Rules:
#   - Verdict rule line MUST contain "(C1-C8)"
#   - Verdict rule line MUST NOT contain "(C1-C5)" (old incomplete range)
#   - Verdict rule line MUST NOT contain "ALL required items"
#   - All C1-C8 items MUST appear in the Static Checklist section
#
# Usage:
#   bash test_code_review_range.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_test_env
}

trap cleanup EXIT

# ─── Test: AC-3 — Verdict rule references C1-C8 ─────────────────────────────

test_ac3_verdict_range_c1_c8() {
    echo "=== AC-3: Verdict rule references (C1-C8) ==="

    local file="$AGENTS_DIR/code-review-evaluator.md"
    setup_test_env

    # Verdict rule MUST contain "(C1-C8)"
    # TODO: Adjust grep pattern if the exact bracket style differs
    # (e.g., "C1-C8" with no parens, or "C1–C8" with en-dash)
    if grep -q '(C1-C8)' "$file" 2>/dev/null; then
        echo "PASS: Verdict rule contains (C1-C8)"
    elif grep -q 'C1-C8' "$file" 2>/dev/null; then
        echo "PASS: Verdict rule contains C1-C8 (no parens)"
    else
        echo "FAIL: Verdict rule does NOT contain (C1-C8)" >&2
        # Show the verdict rule line for debugging
        local verdict_line
        verdict_line="$(grep -i 'pass.*only.*if' "$file" 2>/dev/null || echo '(not found)')"
        echo "  Verdict rule line: $verdict_line" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-3 — Verdict rule does NOT reference old range C1-C5 ────────────

test_ac3_no_old_range_c1_c5() {
    echo "=== AC-3: Verdict rule does NOT reference old range (C1-C5) ==="

    local file="$AGENTS_DIR/code-review-evaluator.md"
    setup_test_env

    if grep -q '(C1-C5)' "$file" 2>/dev/null; then
        echo "FAIL: Verdict rule still references old range (C1-C5)" >&2
        grep -n '(C1-C5)' "$file" 2>/dev/null || true
        return 1
    fi

    echo "PASS: No reference to old range (C1-C5)"
    return 0
}

# ─── Test: AC-3 — No "required" qualifier in verdict rule ────────────────────

test_ac3_no_required_qualifier() {
    echo "=== AC-3: Verdict rule does not use 'ALL required items' ==="

    local file="$AGENTS_DIR/code-review-evaluator.md"
    setup_test_env

    if grep -qi 'ALL required items' "$file" 2>/dev/null; then
        echo "FAIL: Verdict rule uses 'ALL required items'" >&2
        grep -n -i 'ALL required items' "$file" 2>/dev/null || true
        return 1
    fi

    echo "PASS: No 'ALL required items' qualifier"
    return 0
}

# ─── Test: AC-3 — Static Checklist contains all items C1 through C8 ──────────

test_ac3_checklist_contains_all_items() {
    echo "=== AC-3: Static Checklist contains items C1 through C8 ==="

    local file="$AGENTS_DIR/code-review-evaluator.md"
    setup_test_env

    local all_found=true
    for i in $(seq 1 8); do
        local item_id="C$i"
        if grep -q "$item_id" "$file" 2>/dev/null; then
            echo "  Found: $item_id"
        else
            echo "FAIL: $item_id NOT found in code-review-evaluator.md" >&2
            all_found=false
        fi
    done

    if [[ "$all_found" == "true" ]]; then
        echo "PASS: All items C1-C8 found in Static Checklist"
        return 0
    else
        return 1
    fi
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
echo "  AC-3: Code Review Evaluator Range Verification"
echo "=============================================="

run_test "Verdict rule references C1-C8" test_ac3_verdict_range_c1_c8
run_test "No old range C1-C5" test_ac3_no_old_range_c1_c5
run_test "No required qualifier" test_ac3_no_required_qualifier
run_test "Checklist contains C1-C8" test_ac3_checklist_contains_all_items

# ─── Summary ─────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
