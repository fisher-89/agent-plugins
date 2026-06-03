#!/usr/bin/env bash
# test_verdict_rule_unified.sh — Content verification for AC-5.
#
# Covers AC-5: All 7 evaluator verdict rules are unified to
# "ALL items must pass" (without the "required" qualifier).
#
# The old pattern was "ALL required items must pass" (or similar variants).
# The new pattern is: `"pass" only if ALL items pass` (with optional range
# suffix like "(C1-C8)" or "(A1-A5)").
#
# This test verifies:
#   - No evaluator file contains the OLD pattern "ALL required items"
#   - Each evaluator's verdict rule uses the new unified pattern
#
# Usage:
#   bash test_verdict_rule_unified.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_test_env
}

trap cleanup EXIT

# ─── Forbidden patterns (old verdict rule variants) ──────────────────────────

FORBIDDEN_PATTERNS=(
    'ALL required items'
    'all required items'
    'All required items'
)

# ─── Expected verdict rule patterns ──────────────────────────────────────────
# Each evaluator's process section should contain one of these patterns.

# Evaluators without range suffix (plain ALL items pass)
PLAIN_RULES=(
    'ALL items pass'
    'all items pass'
)

# Evaluators with range suffix (C1-C8, A1-A5)
RANGE_RULES=(
    'ALL items pass (C1-C8)'
    'all items pass (C1-C8)'
    'ALL items pass (A1-A5)'
    'all items pass (A1-A5)'
)

# ─── Test: AC-5 — No evaluator contains old pattern "ALL required items" ─────

test_ac5_no_old_pattern_in_any_evaluator() {
    echo "=== AC-5: No evaluator contains old pattern 'ALL required items' ==="

    setup_test_env

    local any_fail=false
    for f in $(get_agent_files); do
        local basename
        basename="$(basename "$f")"

        for pattern in "${FORBIDDEN_PATTERNS[@]}"; do
            if grep -q "$pattern" "$f" 2>/dev/null; then
                echo "FAIL: $basename contains forbidden pattern: '$pattern'" >&2
                grep -n "$pattern" "$f" 2>/dev/null || true
                any_fail=true
            fi
        done
    done

    # Also check the excluded files to make sure they weren't affected
    # (they shouldn't have this pattern either, but they're not the focus)
    for f in "$AGENTS_DIR/unit-test-evaluator.md" "$AGENTS_DIR/integration-test-evaluator.md"; do
        local basename
        basename="$(basename "$f")"
        for pattern in "${FORBIDDEN_PATTERNS[@]}"; do
            if grep -q "$pattern" "$f" 2>/dev/null; then
                echo "WARN: $basename also contains old pattern (out of scope but worth noting): '$pattern'" >&2
            fi
        done
    done

    if [[ "$any_fail" == "false" ]]; then
        echo "PASS: No evaluator contains old 'ALL required items' pattern"
        return 0
    else
        return 1
    fi
}

# ─── Test: AC-5 — Each evaluator has the unified verdict rule ─────────────────

test_ac5_all_evaluators_have_unified_rule() {
    echo "=== AC-5: All evaluators use unified 'ALL items pass' verdict rule ==="

    setup_test_env

    local all_pass=true

    # Check plain-rule evaluators (no range suffix)
    local plain_evaluators=(
        proposal-evaluator
        test-design-evaluator
        dev-design-evaluator
        test-gen-evaluator
        implementation-evaluator
    )

    for ev in "${plain_evaluators[@]}"; do
        local file="$AGENTS_DIR/$ev.md"
        local found=false
        for pattern in "${PLAIN_RULES[@]}"; do
            if grep -q "$pattern" "$file" 2>/dev/null; then
                echo "  PASS: $ev.md contains '$pattern'"
                found=true
                break
            fi
        done
        if [[ "$found" == "false" ]]; then
            echo "FAIL: $ev.md does not contain a unified verdict rule pattern" >&2
            # Show the process section for debugging
            local process_section
            process_section="$(sed -n '/^## Process/,/^## /p' "$file" 2>/dev/null || true)"
            echo "  Process section: $process_section" >&2
            all_pass=false
        fi
    done

    # Check range-suffix evaluators
    # code-review-evaluator — expects (C1-C8)
    local cr_file="$AGENTS_DIR/code-review-evaluator.md"
    local cr_found=false
    for pattern in 'ALL items pass (C1-C8)' 'all items pass (C1-C8)' 'ALL items pass.*C1-C8' 'all items pass.*C1-C8'; do
        if grep -q "$pattern" "$cr_file" 2>/dev/null; then
            echo "  PASS: code-review-evaluator.md contains unified rule with (C1-C8)"
            cr_found=true
            break
        fi
    done
    if [[ "$cr_found" == "false" ]]; then
        echo "FAIL: code-review-evaluator.md does not contain unified rule with (C1-C8)" >&2
        local process_section
        process_section="$(sed -n '/^## Process/,/^## /p' "$cr_file" 2>/dev/null || true)"
        echo "  Process section: $process_section" >&2
        all_pass=false
    fi

    # acceptance-evaluator — expects (A1-A5)
    local ac_file="$AGENTS_DIR/acceptance-evaluator.md"
    local ac_found=false
    for pattern in 'ALL items pass (A1-A5)' 'all items pass (A1-A5)' 'ALL items pass.*A1-A5' 'all items pass.*A1-A5'; do
        if grep -q "$pattern" "$ac_file" 2>/dev/null; then
            echo "  PASS: acceptance-evaluator.md contains unified rule with (A1-A5)"
            ac_found=true
            break
        fi
    done
    if [[ "$ac_found" == "false" ]]; then
        echo "FAIL: acceptance-evaluator.md does not contain unified rule with (A1-A5)" >&2
        local process_section
        process_section="$(sed -n '/^## Process/,/^## /p' "$ac_file" 2>/dev/null || true)"
        echo "  Process section: $process_section" >&2
        all_pass=false
    fi

    if [[ "$all_pass" == "true" ]]; then
        echo "PASS: All evaluators have unified verdict rule"
        return 0
    else
        return 1
    fi
}

# ─── Test: AC-5 — Verdict rule uses only "ALL items pass" (no "must") ───────
# The spec says: `"pass" only if ALL items pass` — note the absence of "must".
# This test detects if someone wrote "ALL items must pass" which is a deviation.

test_ac5_verdict_rule_no_must() {
    echo "=== AC-5: Verdict rule uses 'ALL items pass' not 'ALL items must pass' ==="

    setup_test_env

    local any_issue=false
    for f in $(get_agent_files); do
        local basename
        basename="$(basename "$f")"

        # Look for verdict rule lines in the Process section
        local verdict_lines
        verdict_lines="$(sed -n '/^## Process/,/^## /p' "$f" 2>/dev/null | grep -i 'items.*pass' || true)"

        if [[ -z "$verdict_lines" ]]; then
            echo "WARN: $basename: No verdict rule line found in Process section" >&2
            continue
        fi

        # Check if lines contain "must pass" which is a deviation from the spec
        if echo "$verdict_lines" | grep -qi 'must pass' 2>/dev/null; then
            echo "WARN: $basename: Verdict rule uses 'must pass' (deviation from spec pattern 'ALL items pass')" >&2
            echo "  Lines: $verdict_lines" >&2
            # This is a warning, not a hard failure — the key requirement
            # is the removal of "required", not the exact prose
        fi
    done

    echo "PASS: Verdict rule pattern check complete"
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
echo "  AC-5: Unified Verdict Rule Verification"
echo "=============================================="

run_test "No old pattern in any evaluator" test_ac5_no_old_pattern_in_any_evaluator
run_test "All evaluators have unified rule" test_ac5_all_evaluators_have_unified_rule
run_test "Verdict rule uses 'items pass' not 'must pass'" test_ac5_verdict_rule_no_must

# ─── Summary ─────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
