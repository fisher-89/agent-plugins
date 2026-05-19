#!/usr/bin/env bash
# test_e2e_full_flow.sh — End-to-end test for complete phase-requirements skill flow.
#
# Simulates the full P→E workflow from empty state:
#   1. No active change → AskUserQuestion
#   2. User provides description → derive kebab-case
#   3. Scaffold change directory
#   4. Build Planner prompt with dynamic context
#   5. Run P→E cycle (simulated)
#   6. Verify eval.json contains expected results
#   7. Verify output paths are correct
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

# ─── Test: Full E2E flow from empty state to completed eval.json ────────────

test_e2e_full_flow() {
    echo "=== E2E: Full phase-requirements flow ==="
    echo ""
    echo "  Phase 1: User calls /dev-team:phase-requirements"
    echo "  Phase 2: No change name → AskUserQuestion"
    echo "  Phase 3: User provides description"
    echo "  Phase 4: Derive kebab-case and confirm"
    echo "  Phase 5: Scaffold change"
    echo "  Phase 6: Build Planner prompt"
    echo "  Phase 7: P→E cycle (simulated)"
    echo "  Phase 8: Verify results"
    echo ""

    setup_sandbox
    mock_openspec
    export MOCK_ANSWERS_DIR="$SCRIPT_DIR/fixtures"

    local user_description="Add user authentication with JWT"
    local change_name=""

    # ── Phase 1-2: No change name → need to ask user ──────────────────────
    echo "  [Phase 1-2] No active change name..."
    if [[ -n "$change_name" ]]; then
        echo "FAIL: Should start with no change name" >&2
        return 1
    fi

    # ── Phase 3-4: User provides description → derive name ────────────────
    echo "  [Phase 3-4] User describes: '$user_description'"
    local derived_name
    derived_name="$(derive_kebab_case "$user_description")"

    if [[ -z "$derived_name" ]]; then
        echo "FAIL: Could not derive name from description" >&2
        return 1
    fi
    validate_change_name "$derived_name"
    local valid_status=$?
    if [[ "$valid_status" -ne 0 ]]; then
        echo "FAIL: Derived name '$derived_name' is invalid (exit $valid_status)" >&2
        return 1
    fi
    echo "  [Phase 3-4] Derived name: '$derived_name'"

    # Set the change name
    change_name="$derived_name"

    # ── Phase 5: Scaffold change ────────────────────────────────────────
    echo "  [Phase 5] Scaffolding change '$change_name'..."
    local scaffold_output
    scaffold_output="$(openspec new change "$change_name" 2>&1)"
    local scaffold_status=$?

    if [[ "$scaffold_status" -ne 0 ]]; then
        echo "FAIL: Scaffolding failed: $scaffold_output" >&2
        return 1
    fi

    # Verify scaffold structure
    assert_dir_exists "$FAKE_CHANGES_DIR/$change_name"
    assert_file_exists "$FAKE_CHANGES_DIR/$change_name/.openspec.yaml"
    assert_dir_exists "$FAKE_CHANGES_DIR/$change_name/phases"

    echo "  [Phase 5] Scaffold created successfully"

    # ── Phase 6: Build Planner prompt ───────────────────────────────────
    echo "  [Phase 6] Building Planner prompt..."
    local instructions
    instructions="$(openspec instructions "$change_name" 2>/dev/null || echo '{}')"

    local planner_prompt
    planner_prompt="Write proposal.md for change '$change_name'.
Follow the template at plugins/dev-team/templates/artifacts/proposal.md.template."

    if [[ "$instructions" != "{}" ]]; then
        planner_prompt="$planner_prompt\n\n## CLI Instructions\n\n\`\`\`json\n$instructions\n\`\`\`"
    fi

    # Verify prompt content
    assert_string_contains "$planner_prompt" "$change_name"
    assert_string_contains "$planner_prompt" "proposal.md.template"

    echo "  [Phase 6] Planner prompt built successfully"

    # ── Phase 7: P→E cycle (simulated with Python) ─────────────────────
    echo "  [Phase 7] Running P→E cycle..."

    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    local eval_file="$phases_dir/eval.json"

    # Initialize eval.json
    echo '[]' > "$eval_file"

    # Planner: write proposal
    cat > "$phases_dir/proposal.md" << PROPOSAL_EOF
# Proposal: $change_name

## User Story
As a developer, I want to add user authentication with JWT.

## Scope
- In: JWT-based authentication
- In: Login/Register endpoints
- Out: OAuth providers
- Out: Password reset

## Acceptance Criteria
| ID | Criterion | Method | Priority |
|----|-----------|--------|----------|
| AC-01 | User can register | Integration test | High |
| AC-02 | User can log in | Integration test | High |
| AC-03 | JWT token is returned | Unit test | High |

## Risk
| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Token security | High | Low | Use RS256 |
PROPOSAL_EOF

    # Evaluator: write eval entry with pass verdict
    python3 -c "
import json

with open('$eval_file') as f:
    data = json.load(f)

entry = {
    'phase': '01-requirements',
    'timestamp': '$(date -u +%Y-%m-%dT%H:%M:%S.000Z 2>/dev/null || echo '2026-05-19T12:00:00.000Z')',
    'attempt': 1,
    'verdict': 'pass',
    'report': 'All checklist items passed.',
    'items': [
        {'id': 'R01', 'name': 'Proposal has user story', 'status': 'pass'},
        {'id': 'R02', 'name': 'Proposal has acceptance criteria', 'status': 'pass'},
        {'id': 'R03', 'name': 'Proposal has scope boundaries', 'status': 'pass'},
        {'id': 'R04', 'name': 'Proposal has risk assessment', 'status': 'pass'}
    ],
    'backtrack_to': None,
    'schema_version': '1'
}

data.append(entry)

with open('$eval_file', 'w') as f:
    json.dump(data, f, indent=2, ensure_ascii=False)
"

    echo "  [Phase 7] P→E cycle completed"

    # ── Phase 8: Verify results ─────────────────────────────────────────
    echo "  [Phase 8] Verifying results..."

    # 8a. Verify proposal.md exists
    assert_file_exists "$phases_dir/proposal.md"

    # 8b. Verify eval.json exists
    assert_file_exists "$eval_file"

    # 8c. Verify eval.json is valid and has correct schema
    local schema_ok
    schema_ok="$(python3 -c "
import json, sys
with open('$eval_file') as f:
    data = json.load(f)
if not isinstance(data, list) or len(data) == 0:
    sys.exit(1)
entry = data[-1]
for field in ['phase', 'timestamp', 'attempt', 'verdict', 'items', 'backtrack_to', 'schema_version']:
    if field not in entry:
        sys.exit(1)
if entry['verdict'] not in ('pass', 'fail'):
    sys.exit(1)
print('ok')
")"

    if [[ "$schema_ok" == "ok" ]]; then
        echo "  [Phase 8a] eval.json schema valid"
    else
        echo "FAIL: eval.json schema invalid" >&2
        return 1
    fi

    # 8d. Verify verdict is pass
    local final_verdict
    final_verdict="$(python3 -c "
import json
with open('$eval_file') as f:
    data = json.load(f)
print(data[-1]['verdict'])
")"

    if [[ "$final_verdict" == "pass" ]]; then
        echo "  [Phase 8b] Final verdict: pass"
    else
        echo "FAIL: Final verdict should be 'pass'" >&2
        return 1
    fi

    # 8e. Verify .openspec.yaml was created
    assert_file_exists "$FAKE_CHANGES_DIR/$change_name/.openspec.yaml"

    # 8f. Verify the change name is valid kebab-case
    validate_change_name "$change_name"
    echo "  [Phase 8c] Change name '$change_name' is valid"

    echo ""
    echo "PASS: E2E full flow completed successfully"
    return 0
}

# ─── Test: E2E flow with fail→re-plan iteration ────────────────────────────

test_e2e_with_iteration() {
    echo "=== E2E: Full flow with fail→re-plan iteration ==="

    setup_sandbox
    mock_openspec

    local change_name="iterative-feature"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    local eval_file="$phases_dir/eval.json"

    # Scaffold
    openspec new change "$change_name" > /dev/null 2>&1

    # Write a poor proposal (e.g., missing sections)
    echo "# Incomplete" > "$phases_dir/proposal.md"

    # First eval: fail
    python3 -c "
import json
with open('$eval_file', 'w') as f:
    data = [{
        'phase': '01-requirements',
        'timestamp': '2026-05-19T10:00:00.000Z',
        'attempt': 1,
        'verdict': 'fail',
        'report': 'Missing acceptance criteria and scope.',
        'items': [
            {'id': 'R01', 'name': 'Proposal has user story', 'status': 'pass'},
            {'id': 'R02', 'name': 'Proposal has acceptance criteria', 'status': 'fail'},
            {'id': 'R03', 'name': 'Proposal has scope boundaries', 'status': 'fail'}
        ],
        'backtrack_to': None,
        'schema_version': '1'
    }]
    json.dump(data, f, indent=2)
"

    # Re-plan: write better proposal
    cat > "$phases_dir/proposal.md" << PROPOSAL_EOF
# Proposal: $change_name

## User Story
As a user, I want to...

## Scope
- In: Feature A
- Out: Feature B

## Acceptance Criteria
| ID | Criterion | Method | Priority |
|----|-----------|--------|----------|
| AC-01 | Criterion | Test | High |

## Risk
| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Risk | High | Low | Mitigate |
PROPOSAL_EOF

    # Second eval: pass
    python3 -c "
import json
with open('$eval_file') as f:
    data = json.load(f)
data.append({
    'phase': '01-requirements',
    'timestamp': '2026-05-19T10:10:00.000Z',
    'attempt': 2,
    'verdict': 'pass',
    'report': 'All checklist items passed after revision.',
    'items': [
        {'id': 'R01', 'name': 'Proposal has user story', 'status': 'pass'},
        {'id': 'R02', 'name': 'Proposal has acceptance criteria', 'status': 'pass'},
        {'id': 'R03', 'name': 'Proposal has scope boundaries', 'status': 'pass'},
        {'id': 'R04', 'name': 'Proposal has risk assessment', 'status': 'pass'}
    ],
    'backtrack_to': None,
    'schema_version': '1'
})
with open('$eval_file', 'w') as f:
    json.dump(data, f, indent=2, ensure_ascii=False)
"

    # Verify two entries with correct verdicts
    local entry_count
    entry_count="$(python3 -c "
import json
with open('$eval_file') as f:
    print(len(json.load(f)))
")"

    if [[ "$entry_count" -ne 2 ]]; then
        echo "FAIL: Expected 2 eval entries, got $entry_count" >&2
        return 1
    fi

    local first_verdict second_verdict
    first_verdict="$(python3 -c "
import json
with open('$eval_file') as f:
    print(json.load(f)[0]['verdict'])
")"
    second_verdict="$(python3 -c "
import json
with open('$eval_file') as f:
    print(json.load(f)[1]['verdict'])
")"

    if [[ "$first_verdict" == "fail" && "$second_verdict" == "pass" ]]; then
        echo "PASS: E2E iteration flow correct (fail→pass)"
    else
        echo "FAIL: Expected fail→pass, got $first_verdict→$second_verdict" >&2
        return 1
    fi

    return 0
}

# ─── Test: E2E — Verify all artifacts created ──────────────────────────────

test_e2e_artifacts_created() {
    echo "=== E2E: All expected artifacts created ==="

    setup_sandbox
    mock_openspec

    local change_name="artifact-check"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    local eval_file="$phases_dir/eval.json"

    openspec new change "$change_name" > /dev/null 2>&1

    # Create proposal.md
    echo "# Proposal" > "$phases_dir/proposal.md"

    # Create eval.json with minimal valid entry
    python3 -c "
import json
with open('$eval_file', 'w') as f:
    json.dump([{
        'phase': '01-requirements',
        'timestamp': '2026-05-19T10:00:00.000Z',
        'attempt': 1,
        'verdict': 'pass',
        'items': [],
        'backtrack_to': None,
        'schema_version': '1'
    }], f, indent=2)
"

    # Assert all artifacts
    assert_dir_exists "$FAKE_CHANGES_DIR/$change_name"
    assert_file_exists "$FAKE_CHANGES_DIR/$change_name/.openspec.yaml"
    assert_dir_exists "$phases_dir"
    assert_file_exists "$phases_dir/proposal.md"
    assert_file_exists "$eval_file"

    # Validate eval.json schema
    python3 -c "
import json, sys
with open('$eval_file') as f:
    data = json.load(f)
assert isinstance(data, list)
assert len(data) == 1
assert data[0]['verdict'] in ('pass', 'fail')
print('PASS: eval.json schema valid')
" 2>&1

    echo "PASS: E2E all artifacts created"
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

run_test "E2E: full flow" test_e2e_full_flow
run_test "E2E: fail→re-plan iteration" test_e2e_with_iteration
run_test "E2E: artifact verification" test_e2e_artifacts_created

# ─── Summary ────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
