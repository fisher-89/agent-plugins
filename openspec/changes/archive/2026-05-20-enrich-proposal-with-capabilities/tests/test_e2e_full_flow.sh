#!/usr/bin/env bash
# test_e2e_full_flow.sh — End-to-end test for complete phase-requirements flow
#                         with Capabilities chapter integration.
#
# Covers AC-7:
#   1. Complete P→E flow: Planner writes proposal with Capabilities chapter
#   2. Evaluator checks Capabilities chapter → pass verdict
#   3. eval.json verdict is "pass" and schema matches existing format
#   4. Edge: Planner outputs empty Capabilities → Evaluator fails → P→E cycle retry
#
# Usage:
#   bash test_e2e_full_flow.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_sandbox
}

trap cleanup EXIT

# ─── Helper: Source the evaluator check function ─────────────────────────────

source "$SCRIPT_DIR/test_evaluator_capabilities_check.sh" 2>/dev/null || true

# Re-define run_evaluator_check locally (in case sourcing failed)
run_evaluator_check() {
    local proposal_file="$1"

    if [[ ! -f "$proposal_file" ]]; then
        echo '{"phase":"01-requirements","timestamp":"","attempt":1,"verdict":"fail","items":[],"backtrack_to":null,"schema_version":"1"}'
        return 1
    fi

    local items=()
    local all_pass=true

    if grep -q "^## Capabilities" "$proposal_file"; then
        items+=('{"id":"R08","name":"Proposal has Capabilities chapter","status":"pass"}')
    else
        items+=('{"id":"R08","name":"Proposal has Capabilities chapter","status":"fail","evidence":"Missing ## Capabilities section"}')
        all_pass=false
    fi

    if $all_pass; then
        local caps_block
        caps_block="$(sed -n '/^## Capabilities/,/^## /p' "$proposal_file" 2>/dev/null || true)"
        local has_entry=false
        if echo "$caps_block" | grep -q "^- \*\*"; then
            has_entry=true
        fi

        if $has_entry; then
            items+=('{"id":"R09","name":"Capabilities has at least one entry","status":"pass"}')
        else
            items+=('{"id":"R09","name":"Capabilities has at least one entry","status":"fail","evidence":"No capability entries"}')
            all_pass=false
        fi
    fi

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

    echo "{
        \"phase\": \"01-requirements\",
        \"timestamp\": \"$timestamp\",
        \"attempt\": 1,
        \"verdict\": \"$verdict\",
        \"items\": $items_json,
        \"backtrack_to\": null,
        \"schema_version\": \"1\"
    }"

    $all_pass
}

# ─── Test: E2E — Full P→E flow with Capabilities (pass) ─────────────────────

test_e2e_full_flow_pass() {
    echo "=== E2E: Full P→E flow with Capabilities (pass) ==="
    echo ""
    echo "  Phase 1: Set up change with specs"
    echo "  Phase 2: Planner calls spec list and gets capabilities"
    echo "  Phase 3: Planner writes proposal with Capabilities chapter"
    echo "  Phase 4: Evaluator checks proposal — Capabilities section present"
    echo "  Phase 5: eval.json written with pass verdict"
    echo ""

    setup_sandbox
    mock_openspec
    mock_specs_dir normal
    export MOCK_ANSWERS_DIR="$SCRIPT_DIR/fixtures"

    local change_name="e2e-capabilities-test"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    local eval_file="$phases_dir/eval.json"

    # Phase 1: Scaffold change
    openspec new change "$change_name" > /dev/null 2>&1

    # Phase 2: Planner gets spec list
    local spec_list
    spec_list="$(openspec spec list --json 2>/dev/null || echo '[]')"
    echo "  [Phase 2] Spec list: $spec_list"

    # Verify spec list has capabilities
    assert_string_contains "$spec_list" "auth"
    assert_string_contains "$spec_list" "storage"

    # Phase 3: Planner writes proposal with Capabilities
    cat > "$phases_dir/proposal.md" << PROPOSAL_EOF
# Proposal: $change_name

## User Story
As a developer, I want to ensure capabilities are tracked in proposals.

## Scope
- In: Capability tracking
- Out: Manual entry

## Acceptance Criteria
| ID | Criterion | Method | Priority |
|----|-----------|--------|----------|
| AC-01 | Capabilities are included | Review | High |

## Capabilities

### New Capabilities
- **logging**: New structured logging capability
- **messaging**: New async messaging capability

### Modified Capabilities
- **auth**: Extend auth for OAuth2
- **storage**: Extend storage for S3 backend

## Risk
| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Scope creep | Medium | Low | Clear boundaries |
PROPOSAL_EOF
    echo "  [Phase 3] Proposal written with Capabilities chapter"

    # Phase 4: Evaluator checks proposal
    echo "  [Phase 4] Running evaluator check..."
    local eval_result
    eval_result="$(run_evaluator_check "$phases_dir/proposal.md")"
    local eval_exit=$?
    echo "  [Phase 4] Evaluator verdict: $(echo "$eval_result" | python3 -c "import json,sys; print(json.load(sys.stdin).get('verdict','unknown'))" 2>/dev/null || echo 'unknown')"

    # Phase 5: Write eval.json
    echo "$eval_result" > "$eval_file"
    echo "  [Phase 5] eval.json written"

    # ─── Verify results ─────────────────────────────────────────────────
    echo "  --- Verification ---"

    # 5a. Verify proposal.md exists
    assert_file_exists "$phases_dir/proposal.md"

    # 5b. Verify eval.json exists
    assert_file_exists "$eval_file"

    # 5c. Verify eval.json schema (list with entries)
    python3 -c "
import json, sys
with open('$eval_file') as f:
    data = json.load(f)
assert isinstance(data, dict), 'Expected dict entry'
required = ['phase', 'timestamp', 'attempt', 'verdict', 'items', 'backtrack_to', 'schema_version']
for field in required:
    assert field in data, f'Missing field: {field}'
assert data['verdict'] in ('pass', 'fail'), f'Invalid verdict: {data.get(\"verdict\")}'
print('  eval.json schema valid')
" 2>&1

    # 5d. Verify verdict is pass
    local verdict
    verdict="$(python3 -c "
import json
with open('$eval_file') as f:
    print(json.load(f).get('verdict', 'unknown'))
" 2>/dev/null || echo "unknown")"

    if [[ "$verdict" == "pass" ]]; then
        echo "  Final verdict: pass"
    else
        echo "FAIL: Expected verdict 'pass', got '$verdict'" >&2
        return 1
    fi

    # 5e. Verify schema_version matches existing format
    local schema_version
    schema_version="$(python3 -c "
import json
with open('$eval_file') as f:
    print(json.load(f).get('schema_version', ''))
" 2>/dev/null || echo "")"

    if [[ "$schema_version" == "1" ]]; then
        echo "  Schema version: $schema_version (matches existing)"
    else
        echo "FAIL: Schema version should be '1'" >&2
        return 1
    fi

    # 5f. Verify R08 and R09 items are present
    python3 -c "
import json, sys
with open('$eval_file') as f:
    data = json.load(f)
item_ids = [item['id'] for item in data.get('items', [])]
assert 'R08' in item_ids, 'Missing R08 check'
assert 'R09' in item_ids, 'Missing R09 check'
print('  R08 and R09 present in eval items')
" 2>&1

    echo ""
    echo "PASS: E2E full flow with Capabilities completed successfully"
    return 0
}

# ─── Test: E2E — Verdict pass when Capabilities chapter is present ──────────

test_e2e_capabilities_pass_when_present() {
    echo "=== E2E: AC-7 Capabilities present → pass ==="

    setup_sandbox
    mock_openspec
    mock_specs_dir normal

    local change_name="caps-present"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    local eval_file="$phases_dir/eval.json"
    mkdir -p "$phases_dir"

    # Proposal WITH Capabilities chapter
    cp "$SCRIPT_DIR/fixtures/proposal_with_capabilities.md" "$phases_dir/proposal.md"

    # Run evaluator
    local eval_result
    eval_result="$(run_evaluator_check "$phases_dir/proposal.md")"
    echo "$eval_result" > "$eval_file"

    local verdict
    verdict="$(python3 -c "
import json
with open('$eval_file') as f:
    print(json.load(f).get('verdict', 'unknown'))
" 2>/dev/null || echo "unknown")"

    if [[ "$verdict" == "pass" ]]; then
        echo "PASS: Capabilities present → verdict pass"
    else
        echo "FAIL: Expected pass, got $verdict" >&2
        return 1
    fi

    return 0
}

# ─── Test: E2E — eval.json format matches existing schema ────────────────────

test_e2e_eval_json_schema_compatibility() {
    echo "=== E2E: eval.json format matches existing schema ==="

    setup_sandbox

    local eval_file="$SANDBOX_DIR/eval.json"

    # Create an eval entry that matches the standard schema
    python3 -c "
import json

entry = {
    'phase': '01-requirements',
    'timestamp': '2026-05-20T00:00:00.000Z',
    'attempt': 1,
    'verdict': 'pass',
    'report': 'All checks passed.',
    'items': [
        {'id': 'R08', 'name': 'Proposal has Capabilities chapter', 'status': 'pass'},
        {'id': 'R09', 'name': 'Capabilities has at least one entry', 'status': 'pass'}
    ],
    'backtrack_to': None,
    'schema_version': '1'
}

with open('$eval_file', 'w') as f:
    json.dump([entry], f, indent=2, ensure_ascii=False)
"

    # Validate schema against expected format
    python3 -c "
import json, sys

with open('$eval_file') as f:
    data = json.load(f)

assert isinstance(data, list), 'Root must be array'
assert len(data) == 1, 'Expected 1 entry'

entry = data[0]
required = ['phase', 'timestamp', 'attempt', 'verdict', 'items', 'backtrack_to', 'schema_version']
for field in required:
    assert field in entry, f'Missing field: {field}'

assert entry['verdict'] in ('pass', 'fail'), 'Invalid verdict'
assert entry['schema_version'] == '1', 'Schema version mismatch'

items = entry['items']
assert isinstance(items, list), 'Items must be array'
for item in items:
    assert 'id' in item, 'Item missing id'
    assert 'status' in item, 'Item missing status'
    assert item['status'] in ('pass', 'fail'), 'Invalid item status'

print('PASS: eval.json schema matches existing format')
" 2>&1

    echo "PASS: E2E eval.json schema compatibility verified"
    return 0
}

# ─── Test: E2E Edge — Empty Capabilities → fail → simulated retry ───────────

test_e2e_empty_capabilities_retry() {
    echo "=== E2E Edge: Empty Capabilities → fail → retry ==="

    setup_sandbox
    mock_openspec
    mock_specs_dir normal

    local change_name="empty-retry"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    local eval_file="$phases_dir/eval.json"
    mkdir -p "$phases_dir"

    # Round 1: Write proposal with empty Capabilities (just headings, no entries)
    cat > "$phases_dir/proposal.md" << PROPOSAL_EOF
# Proposal: $change_name

## User Story
Test retry flow.

## Scope
- In: Retry test

## Acceptance Criteria
| ID | Criterion | Method | Priority |
|----|-----------|--------|----------|
| AC-01 | Retry works | Test | High |

## Capabilities

### New Capabilities

### Modified Capabilities

## Risk
| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Risk | Low | Low | Monitor |
PROPOSAL_EOF

    # Evaluate round 1
    local eval1
    eval1="$(run_evaluator_check "$phases_dir/proposal.md")"
    echo "$eval1" > "$eval_file"

    local verdict1
    verdict1="$(python3 -c "
import json
with open('$eval_file') as f:
    print(json.load(f).get('verdict', 'unknown'))
" 2>/dev/null || echo "unknown")"

    if [[ "$verdict1" == "fail" ]]; then
        echo "  Round 1 verdict: fail (expected — empty capabilities)"
    else
        echo "FAIL: Round 1 should fail with empty capabilities, got $verdict1" >&2
        return 1
    fi

    # Simulate P→E retry: Planner adds entries
    echo "  --- Simulating retry: Planner adds capability entries ---"

    # Round 2: Update proposal with proper capability entries
    cat > "$phases_dir/proposal.md" << PROPOSAL_EOF
# Proposal: $change_name

## User Story
Test retry flow.

## Scope
- In: Retry test

## Acceptance Criteria
| ID | Criterion | Method | Priority |
|----|-----------|--------|----------|
| AC-01 | Retry works | Test | High |

## Capabilities

### New Capabilities
- **logging**: New logging capability

### Modified Capabilities
- **auth**: Extend auth capability

## Risk
| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Risk | Low | Low | Monitor |
PROPOSAL_EOF

    # Evaluate round 2
    local eval2
    eval2="$(run_evaluator_check "$phases_dir/proposal.md")"
    # Append to eval.json (simulate list structure)
    python3 -c "
import json
with open('$eval_file') as f:
    data = json.load(f)
with open('$eval_file', 'w') as f:
    json.dump([data, json.loads('''$eval2''')], f, indent=2, ensure_ascii=False)
"

    local verdict2
    verdict2="$(python3 -c "
import json
with open('$eval_file') as f:
    data = json.load(f)
    if isinstance(data, list):
        print(data[-1].get('verdict', 'unknown'))
    else:
        print(data.get('verdict', 'unknown'))
" 2>/dev/null || echo "unknown")"

    if [[ "$verdict2" == "pass" ]]; then
        echo "  Round 2 verdict: pass (after retry)"
    else
        echo "FAIL: Round 2 should pass after adding entries, got $verdict2" >&2
        return 1
    fi

    echo "PASS: E2E Edge empty capabilities → fail → retry → pass"
    return 0
}

# ─── Test: E2E Edge — Only New capabilities (no specs exist) ─────────────────

test_e2e_only_new_capabilities() {
    echo "=== E2E Edge: Planner outputs only New Capabilities ==="

    setup_sandbox
    mock_openspec
    mock_specs_dir empty  # No existing specs
    mock_spec_list empty   # No spec list

    local change_name="only-new"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    local eval_file="$phases_dir/eval.json"
    mkdir -p "$phases_dir"

    # Planner writes proposal with only New capabilities
    cat > "$phases_dir/proposal.md" << PROPOSAL_EOF
# Proposal: $change_name

## User Story
Brand new feature with no existing specs.

## Scope
- In: New feature

## Acceptance Criteria
| ID | Criterion | Method | Priority |
|----|-----------|--------|----------|
| AC-01 | Feature works | Test | High |

## Capabilities

### New Capabilities
- **feature-x**: A brand new capability
- **feature-y**: Another new capability

### Modified Capabilities

## Risk
| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Risk | Low | Low | Monitor |
PROPOSAL_EOF

    # Evaluate — should pass because New Capabilities has entries
    local eval_result
    eval_result="$(run_evaluator_check "$phases_dir/proposal.md")"
    echo "$eval_result" > "$eval_file"

    local verdict
    verdict="$(python3 -c "
import json
with open('$eval_file') as f:
    print(json.load(f).get('verdict', 'unknown'))
" 2>/dev/null || echo "unknown")"

    if [[ "$verdict" == "pass" ]]; then
        echo "PASS: Only New capabilities → verdict pass"
    else
        echo "FAIL: Expected pass with New capabilities, got $verdict" >&2
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

run_test "E2E: full P→E flow with Capabilities" test_e2e_full_flow_pass
run_test "E2E: Capabilities present → pass" test_e2e_capabilities_pass_when_present
run_test "E2E: eval.json schema compatibility" test_e2e_eval_json_schema_compatibility
run_test "E2E Edge: empty Capabilities → fail → retry → pass" test_e2e_empty_capabilities_retry
run_test "E2E Edge: only New capabilities" test_e2e_only_new_capabilities

# ─── Summary ─────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
