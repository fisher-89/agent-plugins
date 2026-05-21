#!/usr/bin/env bash
# test_e2e_full_flow.sh — End-to-end test for the complete eval-log workflow.
#
# This script simulates the full flow:
#   1. Build dev-team-bundle.js with esbuild
#   2. Create a temporary change directory
#   3. Write multiple eval-log entries across multiple phases
#   4. Verify gate constraints are enforced
#   5. Verify attempt auto-increment works across phases
#   6. Verify backtrack-to field is written correctly
#   7. Cleanup
#
# Usage:
#   bash test_e2e_full_flow.sh
#
# Exit code: 0 if all tests pass, 1 if any fail.

set -euo pipefail

# ─── Colors for output ──────────────────────────────────────────────────────
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color
PASS_COUNT=0
FAIL_COUNT=0

pass() {
    echo -e "${GREEN}PASS${NC}: $1"
    PASS_COUNT=$((PASS_COUNT + 1))
}

fail() {
    echo -e "${RED}FAIL${NC}: $1" >&2
    FAIL_COUNT=$((FAIL_COUNT + 1))
}

assert_exit_code() {
    local expected="$1"
    local actual="$2"
    local message="$3"
    if [[ "$actual" -eq "$expected" ]]; then
        pass "$message (exit code $expected)"
    else
        fail "$message (expected exit code $expected, got $actual)"
    fi
}

assert_file_exists() {
    if [[ -f "$1" ]]; then
        pass "File exists: $1"
    else
        fail "File does not exist: $1"
    fi
}

assert_string_contains() {
    if [[ "$2" == *"$3"* ]]; then
        pass "$1: contains '$3'"
    else
        fail "$1: does not contain '$3'"
    fi
}

assert_json_field() {
    local file="$1"
    local field="$2"
    local expected="$3"
    local label="$4"

    local actual
    actual="$(python3 -c "import json; d=json.load(open('$file')); print(d.get('$field', '<<MISSING>>'))" 2>/dev/null || echo '<<PARSE_ERROR>>')"

    if [[ "$actual" == "$expected" ]]; then
        pass "$label: $field = $expected"
    else
        fail "$label: $field expected '$expected', got '$actual'"
    fi
}

# ─── Configuration ──────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../../../.." && pwd)"
BUNDLE_DIR="$PROJECT_ROOT/plugins/dev-team/bin"
BUNDLE_FILE="$BUNDLE_DIR/dev-team-bundle.js"

SANDBOX_DIR=""
CLEANUP_SANDBOX=true

# ─── Helpers ────────────────────────────────────────────────────────────────

setup_sandbox() {
    SANDBOX_DIR="$(mktemp -d -t e2e_eval_log_XXXXXX)"
    echo "=== Sandbox: $SANDBOX_DIR ==="

    export FAKE_CHANGES_DIR="$SANDBOX_DIR/openspec/changes"
    export TEST_CHANGE="e2e-test-change"
    export TEST_CHANGE_DIR="$FAKE_CHANGES_DIR/$TEST_CHANGE"
    export TEST_PHASES_DIR="$TEST_CHANGE_DIR/phases"
    mkdir -p "$TEST_PHASES_DIR"
}

teardown_sandbox() {
    if [[ -n "$SANDBOX_DIR" && -d "$SANDBOX_DIR" && "$CLEANUP_SANDBOX" == true ]]; then
        rm -rf "$SANDBOX_DIR"
        echo "=== Cleaned up sandbox ==="
    fi
}

ensure_bundle() {
    if [[ ! -f "$BUNDLE_FILE" ]]; then
        echo "Building dev-team-bundle.js..."
        (cd "$BUNDLE_DIR" && npm run build)
    fi
    if [[ ! -f "$BUNDLE_FILE" ]]; then
        echo "ERROR: Bundle build failed" >&2
        exit 1
    fi
    echo "Using bundle: $BUNDLE_FILE"
}

run_eval_log() {
    node "$BUNDLE_FILE" eval-log "$@"
}

# ─── Tests ──────────────────────────────────────────────────────────────────

test_phase1_create_first_entry() {
    echo ""
    echo "=== Phase 1: Create first entry (01-requirements) ==="

    rm -f "$TEST_PHASES_DIR/eval.json"

    run_eval_log \
        --change "$TEST_CHANGE" \
        --phase 01-requirements \
        --verdict pass \
        --report "All requirements checklist items pass." \
        --items '[{"item_id":"R1","pass":true,"evidence":"User story clearly defined","notes":""}]'

    local ec=$?
    assert_exit_code 0 $ec "Create first entry"
    assert_file_exists "$TEST_PHASES_DIR/eval.json"

    # Verify entry fields
    python3 -c "
import json
data = json.load(open('$TEST_PHASES_DIR/eval.json'))
assert len(data) == 1, f'Expected 1 entry, got {len(data)}'
e = data[0]
assert e['phase'] == '01-requirements', f'Wrong phase: {e[\"phase\"]}'
assert e['verdict'] == 'pass', f'Wrong verdict: {e[\"verdict\"]}'
assert e['attempt'] == 1, f'Wrong attempt: {e[\"attempt\"]}'
assert e['schema_version'] == '1.0', f'Wrong schema_version: {e[\"schema_version\"]}'
assert e['backtrack_to'] is None, f'Wrong backtrack_to: {e[\"backtrack_to\"]}'
print('Entry 1 fields validated')
"
    pass "Entry 1 field validation"
}

test_phase2_add_test_design() {
    echo ""
    echo "=== Phase 2: Add 02-test-design entry ==="

    run_eval_log \
        --change "$TEST_CHANGE" \
        --phase 02-test-design \
        --verdict pass \
        --report "All test design items pass." \
        --items '[{"item_id":"T1","pass":true,"evidence":"Test levels defined","notes":""}]'

    local ec=$?
    assert_exit_code 0 $ec "Add test-design entry"
}

test_phase3_gate_enforcement() {
    echo ""
    echo "=== Phase 3: Gate enforcement ==="

    # Try to write 04-test-gen without 03-dev-proposal — should be rejected
    run_eval_log \
        --change "$TEST_CHANGE" \
        --phase 04-test-gen \
        --verdict pass \
        --report "Should be blocked by gate." \
        --items '[]' \
        2>&1 || true

    local ec=$?
    if [[ "$ec" -eq 0 ]]; then
        # Try reading eval.json to verify no entry was added with phase 04-test-gen
        local has_entry
        has_entry="$(python3 -c "
import json
data = json.load(open('$TEST_PHASES_DIR/eval.json'))
entries = [e for e in data if e.get('phase') == '04-test-gen']
print(len(entries))
")"
        if [[ "$has_entry" -eq 0 ]]; then
            pass "Gate correctly blocked 04-test-gen (entry not written, exit code may vary)"
        else
            fail "Gate did not block 04-test-gen (entry was written)"
        fi
    else
        assert_exit_code 1 $ec "Gate blocks 04-test-gen without prior phases"
    fi
}

test_phase4_add_dev_proposal() {
    echo ""
    echo "=== Phase 4: Add 03-dev-proposal (passing gate) ==="

    run_eval_log \
        --change "$TEST_CHANGE" \
        --phase 03-dev-proposal \
        --verdict pass \
        --report "Dev proposal approved." \
        --items '[]'

    local ec=$?
    assert_exit_code 0 $ec "Add dev-proposal entry (gate should pass)"
}

test_phase5_attempt_auto_increment() {
    echo ""
    echo "=== Phase 5: Attempt auto-increment ==="

    # Write 3 more entries for 01-requirements (it already has 1)
    for i in 2 3 4; do
        run_eval_log \
            --change "$TEST_CHANGE" \
            --phase 01-requirements \
            --verdict pass \
            --report "Requirements re-evaluation $i." \
            --items '[]'
    done

    # Verify attempt numbers
    local attempt4
    attempt4="$(python3 -c "
import json
data = json.load(open('$TEST_PHASES_DIR/eval.json'))
req_entries = [e for e in data if e['phase'] == '01-requirements']
attempts = [e['attempt'] for e in req_entries]
print(attempts[-1])
")"
    if [[ "$attempt4" -eq 4 ]]; then
        pass "Attempt auto-increment: 4 entries for 01-requirements, last attempt = $attempt4"
    else
        fail "Attempt auto-increment: expected last attempt=4, got $attempt4"
    fi

    # Verify cross-phase isolation: 02-test-design still has attempt=1
    local design_attempt
    design_attempt="$(python3 -c "
import json
data = json.load(open('$TEST_PHASES_DIR/eval.json'))
design_entries = [e for e in data if e['phase'] == '02-test-design']
print(design_entries[0]['attempt'])
")"
    if [[ "$design_attempt" -eq 1 ]]; then
        pass "Cross-phase isolation: 02-test-design attempt still 1"
    else
        fail "Cross-phase isolation: expected attempt=1, got $design_attempt"
    fi
}

test_phase6_backtrack_to_field() {
    echo ""
    echo "=== Phase 6: Backtrack-to field ==="

    # Write with backtrack-to
    run_eval_log \
        --change "$TEST_CHANGE" \
        --phase 06-code-review \
        --verdict pass \
        --report "Code review with backtrack." \
        --items '[]' \
        --backtrack-to 03-dev-proposal

    local ec=$?
    assert_exit_code 0 $ec "Write with --backtrack-to"

    # Verify backtrack_to field
    local bt
    bt="$(python3 -c "
import json
data = json.load(open('$TEST_PHASES_DIR/eval.json'))
cr_entries = [e for e in data if e['phase'] == '06-code-review']
print(cr_entries[0].get('backtrack_to', 'NULL'))
")"
    if [[ "$bt" == "03-dev-proposal" ]]; then
        pass "Backtrack-to field: 03-dev-proposal"
    else
        fail "Backtrack-to field: expected 03-dev-proposal, got $bt"
    fi
}

test_phase7_json_format() {
    echo ""
    echo "=== Phase 7: JSON format verification ==="

    # Verify full file is valid JSON
    python3 -c "
import json
data = json.load(open('$TEST_PHASES_DIR/eval.json'))
assert isinstance(data, list), 'Root must be array'
print(f'Total entries: {len(data)}')
" || fail "JSON validation failed"

    pass "eval.json is valid JSON array"

    # Verify 2-space indentation (first content line starts with 2 spaces)
    local first_content
    first_content="$(grep -v '^\[' "$TEST_PHASES_DIR/eval.json" | grep -v '^\]' | grep -v '^$' | head -1)"
    if [[ "$first_content" == "  "* ]]; then
        pass "2-space indentation verified"
    else
        fail "Expected 2-space indentation"
    fi
}

# ─── Main ───────────────────────────────────────────────────────────────────

main() {
    echo "============================================"
    echo "  E2E Full Flow Test: dev-team eval-log"
    echo "============================================"

    # Parse options
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --no-cleanup) CLEANUP_SANDBOX=false; shift ;;
            *) echo "Unknown option: $1"; exit 1 ;;
        esac
    done

    # Setup
    ensure_bundle
    setup_sandbox

    # Run tests
    test_phase1_create_first_entry
    test_phase2_add_test_design
    test_phase3_gate_enforcement
    test_phase4_add_dev_proposal
    test_phase5_attempt_auto_increment
    test_phase6_backtrack_to_field
    test_phase7_json_format

    # Cleanup
    teardown_sandbox

    # Summary
    echo ""
    echo "============================================"
    echo -e "  Results: ${GREEN}$PASS_COUNT passed${NC}, ${RED}$FAIL_COUNT failed${NC}"
    echo "============================================"

    if [[ "$FAIL_COUNT" -gt 0 ]]; then
        exit 1
    fi
}

main "$@"
