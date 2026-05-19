#!/usr/bin/env bash
# test_backtrack.sh — Integration tests for Step 2 backtrack detection.
#
# Covers:
#   AC-08: Backtrack detection logic remains unchanged (regression)
#   Edge: eval.json is corrupted (not valid JSON)
#   Edge: eval.json entries all have backtrack_to = null
#   Edge: No eval.json file exists
#
# Usage:
#   bash test_backtrack.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_sandbox
}

trap cleanup EXIT

# ─── Helper: Check if backtrack is needed ───────────────────────────────────

# Reads eval.json for a change and checks if any entry has backtrack_to = "01-requirements".
# Returns 0 if backtrack needed, 1 otherwise, 2 if file missing/broken.
check_backtrack_needed() {
    local change_dir="$1"
    local eval_file="$change_dir/phases/eval.json"

    if [[ ! -f "$eval_file" ]]; then
        return 2
    fi

    # Try to parse JSON
    if ! python3 -c "import json; json.load(open('$eval_file'))" 2>/dev/null; then
        echo "WARNING: eval.json is not valid JSON" >&2
        return 2
    fi

    # Check for backtrack markers
    local has_backtrack
    has_backtrack="$(python3 -c "
import json
data = json.load(open('$eval_file'))
# Normalize to list if single object
if isinstance(data, dict):
    data = [data]
for entry in data:
    if entry.get('backtrack_to') == '01-requirements':
        print('yes')
        break
else:
    print('no')
")"

    if [[ "$has_backtrack" == "yes" ]]; then
        return 0
    else
        return 1
    fi
}

# ─── Test: AC-08 — Backtrack detected when marker present ───────────────────

test_ac08_backtrack_detected() {
    echo "=== AC-08: Backtrack detected when marker present ==="

    setup_sandbox

    local change_dir="$FAKE_CHANGES_DIR/backtrack-change"
    mkdir -p "$change_dir/phases"

    # Copy the backtrack fixture eval.json
    cp "$SCRIPT_DIR/fixtures/backtrack_change/phases/eval.json" "$change_dir/phases/eval.json"

    check_backtrack_needed "$change_dir"
    local result=$?

    if [[ "$result" -eq 0 ]]; then
        echo "PASS: AC-08 backtrack correctly detected"
    else
        echo "FAIL: AC-08 should have detected backtrack marker (exit $result)" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-08 — No backtrack when markers absent ─────────────────────────

test_ac08_no_backtrack_without_marker() {
    echo "=== AC-08: No backtrack when markers absent ==="

    setup_sandbox

    local change_dir="$FAKE_CHANGES_DIR/normal-change"
    mkdir -p "$change_dir/phases"

    # Copy the normal fixture eval.json (no backtrack markers)
    cp "$SCRIPT_DIR/fixtures/existing_change/phases/eval.json" "$change_dir/phases/eval.json"

    check_backtrack_needed "$change_dir"
    local result=$?

    if [[ "$result" -eq 1 ]]; then
        echo "PASS: AC-08 no backtrack (as expected)"
    else
        echo "FAIL: AC-08 should not detect backtrack (exit $result)" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-08 Edge — Corrupted eval.json ─────────────────────────────────

test_ac08_corrupted_eval_json() {
    echo "=== AC-08 Edge: Corrupted eval.json ==="

    setup_sandbox

    local change_dir="$FAKE_CHANGES_DIR/corrupt-change"
    mkdir -p "$change_dir/phases"

    # Write invalid JSON
    echo "this is not json" > "$change_dir/phases/eval.json"

    check_backtrack_needed "$change_dir"
    local result=$?

    if [[ "$result" -eq 2 ]]; then
        echo "PASS: AC-08 Edge corrupted JSON returns fallback exit code 2"
    else
        echo "FAIL: AC-08 Edge should return 2 for corrupted JSON (got $result)" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-08 Edge — All backtrack_to are null ───────────────────────────

test_ac08_all_backtrack_null() {
    echo "=== AC-08 Edge: All backtrack_to are null ==="

    setup_sandbox

    local change_dir="$FAKE_CHANGES_DIR/null-backtrack"
    mkdir -p "$change_dir/phases"

    # Create eval.json where all backtrack_to are null
    cat > "$change_dir/phases/eval.json" << JSON_EOF
[
  {
    "phase": "01-requirements",
    "timestamp": "2026-05-19T10:00:00.000Z",
    "attempt": 1,
    "verdict": "pass",
    "backtrack_to": null,
    "schema_version": "1"
  },
  {
    "phase": "01-requirements",
    "timestamp": "2026-05-19T10:05:00.000Z",
    "attempt": 2,
    "verdict": "fail",
    "backtrack_to": null,
    "schema_version": "1"
  }
]
JSON_EOF

    check_backtrack_needed "$change_dir"
    local result=$?

    if [[ "$result" -eq 1 ]]; then
        echo "PASS: AC-08 Edge null backtrack_to correctly ignored"
    else
        echo "FAIL: AC-08 Edge should return 1 for null backtrack_to (got $result)" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-08 Edge — No eval.json file ───────────────────────────────────

test_ac08_no_eval_json() {
    echo "=== AC-08 Edge: No eval.json file ==="

    setup_sandbox

    local change_dir="$FAKE_CHANGES_DIR/no-eval"
    mkdir -p "$change_dir/phases"
    # Intentionally not creating eval.json

    check_backtrack_needed "$change_dir"
    local result=$?

    if [[ "$result" -eq 2 ]]; then
        echo "PASS: AC-08 Edge missing eval.json returns fallback exit code 2"
    else
        echo "FAIL: AC-08 Edge should return 2 for missing eval.json (got $result)" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-08 — Backtracked entry has correct phase name ─────────────────

test_ac08_backtrack_phase_name() {
    echo "=== AC-08: Backtrack entry references '01-requirements' ==="

    setup_sandbox

    local change_dir="$FAKE_CHANGES_DIR/verify-phase"
    mkdir -p "$change_dir/phases"

    cp "$SCRIPT_DIR/fixtures/backtrack_change/phases/eval.json" "$change_dir/phases/eval.json"

    local backtrack_phase
    backtrack_phase="$(python3 -c "
import json
data = json.load(open('$change_dir/phases/eval.json'))
for entry in data:
    if entry.get('backtrack_to') == '01-requirements':
        print(entry.get('phase', 'unknown'))
        break
")"

    if [[ "$backtrack_phase" == "01-requirements" ]]; then
        echo "PASS: AC-08 backtrack phase name is '01-requirements'"
    else
        echo "FAIL: AC-08 unexpected backtrack phase '$backtrack_phase'" >&2
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

run_test "AC-08: backtrack detected" test_ac08_backtrack_detected
run_test "AC-08: no backtrack without marker" test_ac08_no_backtrack_without_marker
run_test "AC-08 Edge: corrupted eval.json" test_ac08_corrupted_eval_json
run_test "AC-08 Edge: null backtrack_to" test_ac08_all_backtrack_null
run_test "AC-08 Edge: no eval.json" test_ac08_no_eval_json
run_test "AC-08: backtrack phase name" test_ac08_backtrack_phase_name

# ─── Summary ────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
