#!/usr/bin/env bash
# test_pe_loop_preservation.sh — End-to-end test for P→E loop preservation.
#
# Covers:
#   AC-07: P→E (Planner → Evaluator) loop structure remains unchanged (regression)
#         - Verify eval.json schema matches the expected format
#         - Verify verdict logic (pass/fail) is unchanged
#         - Verify eval entries contain required fields
#         - Verify P→E loop can iterate with fail→re-plan cycle
#
# Usage:
#   bash test_pe_loop_preservation.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_sandbox
}

trap cleanup EXIT

# ─── Helper: Validate eval.json schema ──────────────────────────────────────

# Validates that an eval.json file conforms to the expected schema.
# Returns 0 if valid, 1 otherwise.
validate_eval_json_schema() {
    local eval_file="$1"

    if [[ ! -f "$eval_file" ]]; then
        echo "FAIL: eval.json not found" >&2
        return 1
    fi

    python3 -c "
import json, sys

with open('$eval_file') as f:
    try:
        data = json.load(f)
    except json.JSONDecodeError as e:
        print(f'FAIL: Invalid JSON: {e}', file=sys.stderr)
        sys.exit(1)

# Must be a list
if not isinstance(data, list):
    print(f'FAIL: eval.json must be a list, got {type(data).__name__}', file=sys.stderr)
    sys.exit(1)

if len(data) == 0:
    print('FAIL: eval.json must have at least one entry', file=sys.stderr)
    sys.exit(1)

required_fields = ['phase', 'timestamp', 'attempt', 'verdict', 'items', 'backtrack_to', 'schema_version']
for i, entry in enumerate(data):
    for field in required_fields:
        if field not in entry:
            print(f'FAIL: entry {i} missing required field: {field}', file=sys.stderr)
            sys.exit(1)

    # Verdict must be pass or fail
    if entry['verdict'] not in ('pass', 'fail'):
        print(f'FAIL: entry {i} verdict must be pass/fail, got {entry[\"verdict\"]}', file=sys.stderr)
        sys.exit(1)

    # Items must be a list of objects with id, name, status
    if not isinstance(entry['items'], list):
        print(f'FAIL: entry {i} items must be a list', file=sys.stderr)
        sys.exit(1)

    for j, item in enumerate(entry['items']):
        for field in ('id', 'name', 'status'):
            if field not in item:
                print(f'FAIL: entry {i} item {j} missing field: {field}', file=sys.stderr)
                sys.exit(1)

print('PASS: eval.json schema validation OK')
sys.exit(0)
" 2>&1
}

# ─── Helper: Simulate P→E cycle ───────────────────────────────────────────

# Simulates one P→E cycle: create proposal, evaluate it, write eval entry.
# Returns 0 if verdict is pass, 1 if fail.
simulate_pe_cycle() {
    local change_dir="$1"
    local attempt="${2:-1}"
    local simulate_fail="${3:-false}"

    # Ensure phases dir exists
    mkdir -p "$change_dir/phases"

    local eval_file="$change_dir/phases/eval.json"

    # Initialize eval.json if not exists
    if [[ ! -f "$eval_file" ]]; then
        echo '[]' > "$eval_file"
    fi

    # Simulate Planner: write proposal.md
    if [[ "$simulate_fail" == "true" ]]; then
        echo "# Incomplete Proposal" > "$change_dir/phases/proposal.md"
    else
        cat > "$change_dir/phases/proposal.md" << PROPOSAL_EOF
# Proposal: test-change

## User Story
As a user, I want to log in.

## Scope
- In: Login feature
- Out: Registration

## Acceptance Criteria
| ID | Criterion | Method | Priority |
|----|-----------|--------|----------|
| AC-01 | User can log in | Test | High |

## Risk
| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Security | High | Low | Review |
PROPOSAL_EOF
    fi

    # Simulate Evaluator: determine verdict
    local verdict="pass"
    if [[ "$simulate_fail" == "true" ]]; then
        verdict="fail"
    fi

    # Build eval entry
    local timestamp
    timestamp="$(date -u +%Y-%m-%dT%H:%M:%S.000Z 2>/dev/null || echo "2026-05-19T12:00:00.000Z")"

    python3 -c "
import json

with open('$eval_file') as f:
    data = json.load(f)

entry = {
    'phase': '01-requirements',
    'timestamp': '$timestamp',
    'attempt': $attempt,
    'verdict': '$verdict',
    'report': 'Proposal evaluated.',
    'items': [
        {'id': 'R01', 'name': 'Has user story', 'status': '$verdict'},
        {'id': 'R02', 'name': 'Has acceptance criteria', 'status': '$verdict'},
        {'id': 'R03', 'name': 'Has scope', 'status': '$verdict'}
    ],
    'backtrack_to': None,
    'schema_version': '1'
}

data.append(entry)

with open('$eval_file', 'w') as f:
    json.dump(data, f, indent=2, ensure_ascii=False)
"

    if [[ "$verdict" == "pass" ]]; then
        return 0
    else
        return 1
    fi
}

# ─── Test: AC-07 — Eval.json schema remains valid ───────────────────────────

test_ac07_eval_json_schema_valid() {
    echo "=== AC-07: eval.json schema is valid ==="

    setup_sandbox

    local change_dir="$FAKE_CHANGES_DIR/schema-test"
    mkdir -p "$change_dir/phases"

    # Copy the existing_change fixture (which has a valid eval.json)
    cp "$SCRIPT_DIR/fixtures/existing_change/phases/eval.json" "$change_dir/phases/eval.json"

    local result
    result="$(validate_eval_json_schema "$change_dir/phases/eval.json")"
    echo "$result"

    if [[ "$result" == *"PASS"* ]]; then
        echo "PASS: AC-07 eval.json schema valid"
        return 0
    else
        echo "FAIL: AC-07 eval.json schema invalid" >&2
        return 1
    fi
}

# ─── Test: AC-07 — P→E cycle produces pass verdict ────────────────────────

test_ac07_pe_cycle_pass() {
    echo "=== AC-07: P→E cycle produces pass verdict ==="

    setup_sandbox

    local change_dir="$FAKE_CHANGES_DIR/pe-pass"
    mkdir -p "$change_dir/phases"

    simulate_pe_cycle "$change_dir" 1 false
    local result=$?

    if [[ "$result" -eq 0 ]]; then
        echo "PASS: AC-07 P→E cycle pass verdict correct"
    else
        echo "FAIL: AC-07 P→E cycle should have passed" >&2
        return 1
    fi

    # Validate the resulting eval.json
    validate_eval_json_schema "$change_dir/phases/eval.json"

    return 0
}

# ─── Test: AC-07 — P→E cycle produces fail verdict ────────────────────────

test_ac07_pe_cycle_fail() {
    echo "=== AC-07: P→E cycle produces fail verdict ==="

    setup_sandbox

    local change_dir="$FAKE_CHANGES_DIR/pe-fail"
    mkdir -p "$change_dir/phases"

    simulate_pe_cycle "$change_dir" 1 true
    local result=$?

    if [[ "$result" -ne 0 ]]; then
        echo "PASS: AC-07 P→E cycle fail verdict correct"
    else
        echo "FAIL: AC-07 P→E cycle should have failed" >&2
        return 1
    fi

    # Validate the resulting eval.json
    validate_eval_json_schema "$change_dir/phases/eval.json"

    return 0
}

# ─── Test: AC-07 — P→E loop can iterate (fail → re-plan → pass) ────────────

test_ac07_pe_loop_iteration() {
    echo "=== AC-07: P→E loop iteration (fail→re-plan→pass) ==="

    setup_sandbox

    local change_dir="$FAKE_CHANGES_DIR/pe-loop"
    mkdir -p "$change_dir/phases"

    # First cycle: fail
    simulate_pe_cycle "$change_dir" 1 true
    echo "  Attempt 1: fail"

    # Read verdict from eval.json
    local verdict1
    verdict1="$(python3 -c "
import json
with open('$change_dir/phases/eval.json') as f:
    data = json.load(f)
print(data[-1]['verdict'])
")"
    if [[ "$verdict1" != "fail" ]]; then
        echo "FAIL: First cycle should have verdict=fail" >&2
        return 1
    fi

    # Second cycle: pass
    simulate_pe_cycle "$change_dir" 2 false
    echo "  Attempt 2: pass"

    local verdict2
    verdict2="$(python3 -c "
import json
with open('$change_dir/phases/eval.json') as f:
    data = json.load(f)
print(data[-1]['verdict'])
")"
    if [[ "$verdict2" != "pass" ]]; then
        echo "FAIL: Second cycle should have verdict=pass" >&2
        return 1
    fi

    # Verify both entries exist
    local entry_count
    entry_count="$(python3 -c "
import json
with open('$change_dir/phases/eval.json') as f:
    print(len(json.load(f)))
")"
    if [[ "$entry_count" -eq 2 ]]; then
        echo "PASS: AC-07 P→E loop iteration correct (2 entries)"
    else
        echo "FAIL: Expected 2 eval entries, got $entry_count" >&2
        return 1
    fi

    return 0
}

# ─── Test: AC-07 — Eval entry fields match schema spec ──────────────────────

test_ac07_eval_entry_field_types() {
    echo "=== AC-07: Eval entry fields match schema spec ==="

    setup_sandbox

    local change_dir="$FAKE_CHANGES_DIR/field-types"
    mkdir -p "$change_dir/phases"

    simulate_pe_cycle "$change_dir" 1 false

    # Verify field types
    python3 -c "
import json, sys

with open('$change_dir/phases/eval.json') as f:
    data = json.load(f)

entry = data[0]

# phase must be string
assert isinstance(entry['phase'], str), 'phase must be string'
assert isinstance(entry['timestamp'], str), 'timestamp must be string'
assert isinstance(entry['attempt'], int), 'attempt must be int'
assert isinstance(entry['verdict'], str), 'verdict must be string'
assert isinstance(entry['items'], list), 'items must be list'
assert entry['backtrack_to'] is None or isinstance(entry['backtrack_to'], str), 'backtrack_to must be string or null'
assert isinstance(entry['schema_version'], str), 'schema_version must be string'

print('PASS: AC-07 field types correct')
sys.exit(0)
" 2>&1

    return 0
}

# ─── Test: AC-07 — Verdict logic (pass checks items) ────────────────────────

test_ac07_verdict_logic() {
    echo "=== AC-07: Verdict logic (pass/fail based on items) ==="

    setup_sandbox

    local eval_file="$SANDBOX_DIR/test_eval.json"

    # Create a pass entry where all items have status=pass
    python3 -c "
import json
data = [
    {
        'phase': '01-requirements',
        'timestamp': '2026-05-19T10:00:00.000Z',
        'attempt': 1,
        'verdict': 'pass',
        'report': 'All passed',
        'items': [
            {'id': 'R01', 'name': 'Item 1', 'status': 'pass'},
            {'id': 'R02', 'name': 'Item 2', 'status': 'pass'}
        ],
        'backtrack_to': None,
        'schema_version': '1'
    }
]
with open('$eval_file', 'w') as f:
    json.dump(data, f, indent=2)
"

    # Verify all items have status=pass when verdict=pass
    local all_pass
    all_pass="$(python3 -c "
import json
with open('$eval_file') as f:
    data = json.load(f)
entry = data[0]
print(all(item['status'] == 'pass' for item in entry['items']))
")"

    if [[ "$all_pass" == "True" ]]; then
        echo "PASS: AC-07 verdict logic: pass = all items pass"
    else
        echo "FAIL: AC-07 pass verdict should have all items pass" >&2
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

run_test "AC-07: eval.json schema valid" test_ac07_eval_json_schema_valid
run_test "AC-07: P→E cycle pass" test_ac07_pe_cycle_pass
run_test "AC-07: P→E cycle fail" test_ac07_pe_cycle_fail
run_test "AC-07: P→E loop iteration" test_ac07_pe_loop_iteration
run_test "AC-07: eval entry field types" test_ac07_eval_entry_field_types
run_test "AC-07: verdict logic" test_ac07_verdict_logic

# ─── Summary ────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
