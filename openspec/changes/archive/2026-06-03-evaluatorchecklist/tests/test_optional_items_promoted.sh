#!/usr/bin/env bash
# test_optional_items_promoted.sh — Semantic verification for AC-2.
#
# Covers AC-2: 6 previously-optional items (R7, T5, T8, D7, D9, A5) have been
# promoted to mandatory status. The tests verify that:
#   - Each promoted item ID appears in its evaluator's Static Checklist
#   - The verdict rule does NOT contain any special exemption/exclusion logic
#     for these items (e.g., no "if present" or conditional language)
#   - The verdict rule does NOT reference separate "required" vs "optional" sets
#
# Per the spec: ALL items in the Static Checklist are now mandatory. The verdict
# rule reads `"pass" only if ALL items pass` (with range suffix where applicable).
#
# Usage:
#   bash test_optional_items_promoted.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_test_env
}

trap cleanup EXIT

# ─── Verdict Rule Patterns ───────────────────────────────────────────────────

# Verdict rule must NOT contain "required" as a qualifier on ALL items
# (e.g., "ALL required items" is the OLD pattern that should be gone)
VERDICT_RULE_FORBIDDEN='ALL required items'

# Verdict rule must NOT contain exemption/conditional language for any item
EXEMPTION_PATTERNS=(
    'if present'
    'if applicable'
    'may be skipped'
    'optional'
    'not required'
)

# ─── Test: AC-2 — R7 promoted in proposal-evaluator ─────────────────────────

test_ac2_r7_promoted() {
    echo "=== AC-2: R7 promoted to mandatory in proposal-evaluator.md ==="

    local file="$AGENTS_DIR/proposal-evaluator.md"
    setup_test_env

    # R7 must appear in the Static Checklist
    assert_file_contains "$file" 'R7' "R7 appears in proposal-evaluator checklist"

    # R7 must NOT be referenced with any exemption/conditional language
    # TODO: If the evaluator uses a conditional structure for R7, this will fail
    # and the implementation must be corrected
    for pattern in "${EXEMPTION_PATTERNS[@]}"; do
        # Only check lines mentioning R7
        if grep -q 'R7' "$file" 2>/dev/null; then
            local r7_lines
            r7_lines="$(grep 'R7' "$file" 2>/dev/null || true)"
            if echo "$r7_lines" | grep -qi "$pattern" 2>/dev/null; then
                echo "FAIL: R7 has exemption language: '$pattern'" >&2
                echo "  Lines: $r7_lines" >&2
                return 1
            fi
        fi
    done

    echo "PASS: R7 is mandatory (no exemption language found)"
    return 0
}

# ─── Test: AC-2 — T5 and T8 promoted in test-design-evaluator ───────────────

test_ac2_t5_t8_promoted() {
    echo "=== AC-2: T5 and T8 promoted to mandatory in test-design-evaluator.md ==="

    local file="$AGENTS_DIR/test-design-evaluator.md"
    setup_test_env

    # T5 must appear in the Static Checklist
    assert_file_contains "$file" 'T5' "T5 appears in test-design-evaluator checklist"
    # T8 must appear in the Static Checklist
    assert_file_contains "$file" 'T8' "T8 appears in test-design-evaluator checklist"

    # T5 and T8 must NOT retain "optional" semantics
    for item_id in 'T5' 'T8'; do
        local item_lines
        item_lines="$(grep "$item_id" "$file" 2>/dev/null || true)"
        for pattern in "${EXEMPTION_PATTERNS[@]}"; do
            if echo "$item_lines" | grep -qi "$pattern" 2>/dev/null; then
                echo "FAIL: $item_id has exemption language: '$pattern'" >&2
                echo "  Lines: $item_lines" >&2
                return 1
            fi
        done
    done

    echo "PASS: T5 and T8 are mandatory (no exemption language found)"
    return 0
}

# ─── Test: AC-2 — D7 and D9 promoted in dev-design-evaluator ────────────────

test_ac2_d7_d9_promoted() {
    echo "=== AC-2: D7 and D9 promoted to mandatory in dev-design-evaluator.md ==="

    local file="$AGENTS_DIR/dev-design-evaluator.md"
    setup_test_env

    # D7 must appear in the Static Checklist
    assert_file_contains "$file" 'D7' "D7 appears in dev-design-evaluator checklist"
    # D9 must appear in the Static Checklist
    assert_file_contains "$file" 'D9' "D9 appears in dev-design-evaluator checklist"

    for item_id in 'D7' 'D9'; do
        local item_lines
        item_lines="$(grep "$item_id" "$file" 2>/dev/null || true)"
        for pattern in "${EXEMPTION_PATTERNS[@]}"; do
            if echo "$item_lines" | grep -qi "$pattern" 2>/dev/null; then
                echo "FAIL: $item_id has exemption language: '$pattern'" >&2
                echo "  Lines: $item_lines" >&2
                return 1
            fi
        done
    done

    echo "PASS: D7 and D9 are mandatory (no exemption language found)"
    return 0
}

# ─── Test: AC-2 — A5 promoted in acceptance-evaluator ────────────────────────

test_ac2_a5_promoted() {
    echo "=== AC-2: A5 promoted to mandatory in acceptance-evaluator.md ==="

    local file="$AGENTS_DIR/acceptance-evaluator.md"
    setup_test_env

    # A5 must appear in the Static Checklist
    assert_file_contains "$file" 'A5' "A5 appears in acceptance-evaluator checklist"

    local item_lines
    item_lines="$(grep 'A5' "$file" 2>/dev/null || true)"
    for pattern in "${EXEMPTION_PATTERNS[@]}"; do
        if echo "$item_lines" | grep -qi "$pattern" 2>/dev/null; then
            echo "FAIL: A5 has exemption language: '$pattern'" >&2
            echo "  Lines: $item_lines" >&2
            return 1
        fi
    done

    echo "PASS: A5 is mandatory (no exemption language found)"
    return 0
}

# ─── Test: AC-2 — Verdict rules don't distinguish required vs optional ───────

test_ac2_no_required_qualifier() {
    echo "=== AC-2: No evaluator verdict rule distinguishes required vs optional ==="

    setup_test_env

    local all_pass=true
    for f in $(get_agent_files); do
        local basename
        basename="$(basename "$f")"

        # Check that the verdict rule section doesn't contain "ALL required items"
        if grep -qi "$VERDICT_RULE_FORBIDDEN" "$f" 2>/dev/null; then
            echo "FAIL: $basename contains forbidden pattern: '$VERDICT_RULE_FORBIDDEN'" >&2
            grep -n "$VERDICT_RULE_FORBIDDEN" "$f" 2>/dev/null || true
            all_pass=false
        fi
    done

    if [[ "$all_pass" == "true" ]]; then
        echo "PASS: No evaluator uses 'ALL required items' — all items are mandatory"
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
echo "  AC-2: Optional Items Promoted Verification"
echo "=============================================="

run_test "R7 promoted in proposal-evaluator" test_ac2_r7_promoted
run_test "T5/T8 promoted in test-design-evaluator" test_ac2_t5_t8_promoted
run_test "D7/D9 promoted in dev-design-evaluator" test_ac2_d7_d9_promoted
run_test "A5 promoted in acceptance-evaluator" test_ac2_a5_promoted
run_test "No required qualifier in verdict rules" test_ac2_no_required_qualifier

# ─── Summary ─────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
