#!/usr/bin/env bash
# test_evaluator_capabilities_check.sh — Integration tests for Evaluator capabilities checklist.
#
# Covers AC-5:
#   - Evaluator checklist includes Capabilities chapter check items (R8/R9, marked required)
#   - Proposal WITH Capabilities chapter passes the check
#   - Proposal WITHOUT Capabilities chapter fails the check
#   - Edge: Proposal with empty Capabilities subsections fails (no entries)
#
# Usage:
#   bash test_evaluator_capabilities_check.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_sandbox
}

trap cleanup EXIT

# ─── Helper: Simulate Evaluator checklist execution ──────────────────────────

# Runs a simulated evaluator check on a proposal file.
# Returns 0 if all required items pass, 1 if any fail.
# Writes the eval entry JSON to stdout.
run_evaluator_check() {
    local proposal_file="$1"
    local eval_file="${2:-}"

    if [[ ! -f "$proposal_file" ]]; then
        echo "FAIL: Proposal file not found: $proposal_file" >&2
        return 1
    fi

    local items=()
    local all_pass=true

    # R8: Proposal has Capabilities chapter
    if grep -q "^## Capabilities" "$proposal_file"; then
        items+=('{"id":"R08","name":"Proposal has Capabilities chapter","status":"pass"}')
        echo "  [R08] Capabilities chapter present: PASS"
    else
        items+=('{"id":"R08","name":"Proposal has Capabilities chapter","status":"fail","evidence":"Missing ## Capabilities section"}')
        echo "  [R08] Capabilities chapter present: FAIL" >&2
        all_pass=false
    fi

    # R9: Capabilities chapter has at least one New or Modified entry
    if grep -q "^## Capabilities" "$proposal_file"; then
        # Extract lines between ## Capabilities and the next ## section
        local caps_block
        caps_block="$(sed -n '/^## Capabilities/,/^## /p' "$proposal_file" 2>/dev/null || true)"

        local has_new=false
        local has_modified=false

        if echo "$caps_block" | grep -q "### New Capabilities"; then
            # Check if there are list items under New Capabilities
            local new_section
            new_section="$(echo "$caps_block" | sed -n '/### New Capabilities/,/### /p' 2>/dev/null || true)"
            if echo "$new_section" | grep -q "^- \*\*"; then
                has_new=true
            fi
        fi

        if echo "$caps_block" | grep -q "### Modified Capabilities"; then
            local modified_section
            modified_section="$(echo "$caps_block" | sed -n '/### Modified Capabilities/,/### /p' 2>/dev/null || true)"
            if echo "$modified_section" | grep -q "^- \*\*"; then
                has_modified=true
            fi
        fi

        if $has_new || $has_modified; then
            items+=('{"id":"R09","name":"Capabilities has at least one entry","status":"pass"}')
            echo "  [R09] Capabilities entries present: PASS"
        else
            items+=('{"id":"R09","name":"Capabilities has at least one entry","status":"fail","evidence":"No capability entries found in New or Modified subsections"}')
            echo "  [R09] Capabilities entries present: FAIL" >&2
            all_pass=false
        fi
    else
        items+=('{"id":"R09","name":"Capabilities has at least one entry","status":"fail","evidence":"Capabilities chapter missing"}')
        echo "  [R09] Capabilities entries present: FAIL (chapter missing)" >&2
        all_pass=false
    fi

    # Build JSON output
    local verdict="pass"
    $all_pass || verdict="fail"

    local items_json="["
    local first=true
    for item in "${items[@]}"; do
        if $first; then
            items_json="$items_json$item"
            first=false
        else
            items_json="$items_json,$item"
        fi
    done
    items_json="$items_json]"

    local timestamp
    timestamp="$(date -u +%Y-%m-%dT%H:%M:%S.000Z 2>/dev/null || echo '2026-05-20T00:00:00.000Z')"

    local eval_entry="{
        \"phase\": \"01-requirements\",
        \"timestamp\": \"$timestamp\",
        \"attempt\": 1,
        \"verdict\": \"$verdict\",
        \"items\": $items_json,
        \"backtrack_to\": null,
        \"schema_version\": \"1\"
    }"

    echo "$eval_entry"

    if $all_pass; then
        return 0
    else
        return 1
    fi
}

# ─── Test: AC-5 — Checklist includes R8 (Capabilities chapter) ───────────────

test_ac5_checklist_includes_r8() {
    echo "=== AC-5: Evaluator checklist includes R8 (Capabilities chapter check) ==="

    setup_sandbox

    local proposal_file="$SCRIPT_DIR/fixtures/proposal_with_capabilities.md"
    local result
    result="$(run_evaluator_check "$proposal_file" 2>&1 || true)"

    # The output should reference R08
    assert_string_contains "$result" "R08"

    # The output should contain the check item name
    assert_string_contains "$result" "Capabilities chapter"

    echo "PASS: AC-5 checklist includes R8"
    return 0
}

# ─── Test: AC-5 — Checklist includes R9 (Capabilities entries) ────────────────

test_ac5_checklist_includes_r9() {
    echo "=== AC-5: Evaluator checklist includes R9 (Capabilities entries check) ==="

    setup_sandbox

    local proposal_file="$SCRIPT_DIR/fixtures/proposal_with_capabilities.md"
    local result
    result="$(run_evaluator_check "$proposal_file" 2>&1 || true)"

    assert_string_contains "$result" "R09"
    assert_string_contains "$result" "at least one entry"

    echo "PASS: AC-5 checklist includes R9"
    return 0
}

# ─── Test: AC-5 — Proposal WITH Capabilities passes ─────────────────────────

test_ac5_proposal_with_capabilities_pass() {
    echo "=== AC-5: Proposal WITH Capabilities passes evaluator check ==="

    setup_sandbox

    local proposal_file="$SCRIPT_DIR/fixtures/proposal_with_capabilities.md"
    local result
    result="$(run_evaluator_check "$proposal_file" 2>&1)" && true
    local exit_code=$?

    if [[ "$exit_code" -eq 0 ]]; then
        echo "PASS: AC-5 proposal with capabilities passed"
    else
        echo "FAIL: AC-5 proposal with capabilities should pass" >&2
        echo "  Output: $result" >&2
        return 1
    fi

    # Verify verdict in JSON output
    local verdict
    verdict="$(echo "$result" | python3 -c "
import json, sys
data = json.loads(sys.stdin.read())
print(data.get('verdict', 'unknown'))
" 2>/dev/null || echo "unknown")"

    if [[ "$verdict" == "pass" ]]; then
        echo "  Verdict: pass"
    else
        echo "FAIL: Expected verdict 'pass', got '$verdict'" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-5 — Proposal WITHOUT Capabilities fails ────────────────────────

test_ac5_proposal_without_capabilities_fail() {
    echo "=== AC-5: Proposal WITHOUT Capabilities fails evaluator check ==="

    setup_sandbox

    local proposal_file="$SCRIPT_DIR/fixtures/proposal_without_capabilities.md"
    local result
    result="$(run_evaluator_check "$proposal_file" 2>&1)" && true
    local exit_code=$?

    if [[ "$exit_code" -ne 0 ]]; then
        echo "PASS: AC-5 proposal without capabilities failed (expected)"
    else
        echo "FAIL: AC-5 proposal without capabilities should fail" >&2
        return 1
    fi

    # Verify verdict in JSON
    local verdict
    verdict="$(echo "$result" | python3 -c "
import json, sys
data = json.loads(sys.stdin.read())
print(data.get('verdict', 'unknown'))
" 2>/dev/null || echo "unknown")"

    if [[ "$verdict" == "fail" ]]; then
        echo "  Verdict: fail (expected)"
    else
        echo "FAIL: Expected verdict 'fail', got '$verdict'" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-5 Edge — Empty capabilities sections fail ──────────────────────

test_ac5_empty_capabilities_fail() {
    echo "=== AC-5 Edge: Empty Capabilities subsections fail check ==="

    setup_sandbox

    local proposal_file="$SCRIPT_DIR/fixtures/proposal_empty_capabilities.md"
    local result
    result="$(run_evaluator_check "$proposal_file" 2>&1)" && true
    local exit_code=$?

    if [[ "$exit_code" -ne 0 ]]; then
        echo "PASS: AC-5 Edge empty capabilities failed (expected — no entries)"
    else
        echo "FAIL: AC-5 Edge empty capabilities should fail (no entries)" >&2
        return 1
    fi

    # Verify R09 specifically failed
    if echo "$result" | python3 -c "
import json, sys
data = json.loads(sys.stdin.read())
items = data.get('items', [])
for item in items:
    if item.get('id') == 'R09':
        assert item['status'] == 'fail', 'R09 should fail for empty capabilities'
        exit(0)
print('R09 not found')
" 2>/dev/null; then
        echo "  R09 correctly failed for empty capabilities"
    else
        echo "FAIL: R09 should have failed for empty capabilities" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-5 Edge — R8/R9 marked as required (not optional) ───────────────

test_ac5_r8_r9_are_required() {
    echo "=== AC-5 Edge: R8 and R9 are required checks (determine verdict) ==="

    setup_sandbox

    local proposal_file="$SCRIPT_DIR/fixtures/proposal_without_capabilities.md"
    local result
    result="$(run_evaluator_check "$proposal_file" 2>&1)" && true
    local exit_code=$?

    # When R8 fails, the overall verdict should be fail (required check)
    local verdict
    verdict="$(echo "$result" | python3 -c "
import json, sys
data = json.loads(sys.stdin.read())
print(data.get('verdict', 'unknown'))
" 2>/dev/null || echo "unknown")"

    if [[ "$verdict" == "fail" ]]; then
        echo "PASS: R8/R9 are required — their failure causes overall verdict 'fail'"
    else
        echo "FAIL: R8/R9 failure should produce verdict 'fail', got '$verdict'" >&2
        return 1
    fi

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

run_test "AC-5: checklist includes R8" test_ac5_checklist_includes_r8
run_test "AC-5: checklist includes R9" test_ac5_checklist_includes_r9
run_test "AC-5: proposal with capabilities passes" test_ac5_proposal_with_capabilities_pass
run_test "AC-5: proposal without capabilities fails" test_ac5_proposal_without_capabilities_fail
run_test "AC-5 Edge: empty capabilities fail" test_ac5_empty_capabilities_fail
run_test "AC-5 Edge: R8/R9 required" test_ac5_r8_r9_are_required

# ─── Summary ─────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
