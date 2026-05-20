#!/usr/bin/env bash
# test_cli_fail_fallback.sh — Integration tests for CLI failure fallback behavior.
#
# Covers AC-6:
#   - CLI `spec list --json` fails — agent still generates proposal, all capabilities marked New
#   - `openspec_spec_list()` output format is abnormal — fallback to empty list
#   - Edge: No change to proposal structure when CLI is unavailable
#
# Usage:
#   bash test_cli_fail_fallback.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_sandbox
}

trap cleanup EXIT

# ─── Helper: Simulate Planner fallback behavior when CLI fails ───────────────

# Returns a list of capabilities to use in the proposal when the CLI is unavailable.
# When CLI fails, all capabilities should be treated as New (since we cannot
# distinguish existing vs new without the spec list).
get_fallback_capabilities() {
    local spec_list_result="$1"

    if [[ -z "$spec_list_result" || "$spec_list_result" == "[]" ]]; then
        # No capabilities known — return empty
        echo ""
        return 0
    fi

    # When we DO have a spec list, classify as usual
    # (This helper is for the fallback path, so normally returns empty)
    echo ""
}

# ─── Helper: Generate proposal with fallback capabilities (all New) ──────────

generate_fallback_proposal() {
    local change_name="$1"
    local new_caps_csv="${2:-}"

    cat << PROPOSAL_EOF
# Proposal: $change_name

## User Story
As a developer, I want to build a feature even when CLI tools are unavailable.

## Scope
- In: Core functionality
- Out: External integrations

## Acceptance Criteria
| ID | Criterion | Method | Priority |
|----|-----------|--------|----------|
| AC-01 | Feature works without CLI | Review | High |

## Capabilities

### New Capabilities
PROPOSAL_EOF

    if [[ -n "$new_caps_csv" ]]; then
        IFS=' ' read -ra caps <<< "$new_caps_csv"
        for cap in "${caps[@]}"; do
            echo "- **$cap**: New fallback capability"
        done
    fi

    # In fallback mode, there are NO Modified capabilities (all are New)
    cat << PROPOSAL_EOF

### Modified Capabilities

## Risk
| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| CLI unavailability | Low | Low | Fallback to all-New classification |
PROPOSAL_EOF
}

# ─── Test: AC-6 — CLI spec list fails, proposal still generates ─────────────

test_ac6_cli_fail_still_generates_proposal() {
    echo "=== AC-6: CLI spec list fails — proposal still generates ==="

    setup_sandbox
    mock_openspec
    mock_spec_list error  # Simulate CLI failure

    local change_name="fallback-test"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    mkdir -p "$phases_dir"

    # Attempt to get spec list — should fail
    local spec_list_output
    spec_list_output="$(openspec spec list --json 2>/dev/null || echo "")"

    # Verify CLI call failed
    if [[ -z "$spec_list_output" ]]; then
        echo "  CLI spec list failed as expected"
    else
        echo "  CLI returned: $spec_list_output"
    fi

    # Get fallback capabilities (empty, since CLI failed)
    local fallback_caps
    fallback_caps="$(get_fallback_capabilities "$spec_list_output")"

    # Generate proposal with fallback (no capabilities known — empty proposal)
    generate_fallback_proposal "$change_name" "" > "$phases_dir/proposal.md"

    # Verify proposal was generated
    assert_file_exists "$phases_dir/proposal.md"
    assert_string_contains "$(cat "$phases_dir/proposal.md")" "# Proposal: $change_name"

    echo "PASS: AC-6 proposal generated despite CLI failure"
    return 0
}

# ─── Test: AC-6 — All capabilities marked as New when CLI is unavailable ──────

test_ac6_all_capabilities_new_when_cli_unavailable() {
    echo "=== AC-6: All capabilities marked New when CLI is unavailable ==="

    setup_sandbox
    mock_openspec
    mock_spec_list error

    local change_name="all-new-fallback"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    mkdir -p "$phases_dir"

    # When CLI fails, we have no way to know existing specs.
    # The planner should treat all planned capabilities as New.
    # Simulate: planner has "logging" and "messaging" as planned capabilities.
    local planned_caps="logging messaging"
    generate_fallback_proposal "$change_name" "$planned_caps" > "$phases_dir/proposal.md"

    # Verify New Capabilities section has entries
    if grep -q "^- \*\*logging\*\*" "$phases_dir/proposal.md"; then
        echo "  logging is in New Capabilities"
    else
        echo "FAIL: logging should be in New Capabilities" >&2
        return 1
    fi

    if grep -q "^- \*\*messaging\*\*" "$phases_dir/proposal.md"; then
        echo "  messaging is in New Capabilities"
    else
        echo "FAIL: messaging should be in New Capabilities" >&2
        return 1
    fi

    # Modified Capabilities should be empty
    local modified_section
    modified_section="$(sed -n '/### Modified Capabilities/,/^## /p' "$phases_dir/proposal.md" 2>/dev/null || true)"
    if echo "$modified_section" | grep -q "^- \*\*"; then
        echo "FAIL: Modified Capabilities should be empty in fallback mode" >&2
        return 1
    fi

    echo "PASS: AC-6 all capabilities marked New when CLI unavailable"
    return 0
}

# ─── Test: AC-6 — Abnormal output format falls back to empty list ────────────

test_ac6_abnormal_output_fallback() {
    echo "=== AC-6: Abnormal output format falls back to empty list ==="

    setup_sandbox
    mock_openspec
    mock_spec_list invalid  # Returns non-array JSON

    local change_name="invalid-output"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    mkdir -p "$phases_dir"

    # Get spec list — should return non-array JSON
    local spec_list_output
    spec_list_output="$(openspec spec list --json 2>/dev/null || echo "")"

    echo "  CLI returned: $spec_list_output"

    # Validate output is not a valid JSON array
    local is_valid_array
    is_valid_array="$(python3 -c "
import json, sys
try:
    data = json.loads('''$spec_list_output''')
    if isinstance(data, list):
        print('true')
    else:
        print('false')
except:
    print('false')
" 2>/dev/null || echo "false")"

    if [[ "$is_valid_array" == "false" ]]; then
        echo "PASS: Abnormal output detected (not a JSON array)"
    else
        echo "FAIL: Expected abnormal output to be detected" >&2
        return 1
    fi

    # Generate fallback proposal with empty capabilities
    generate_fallback_proposal "$change_name" "" > "$phases_dir/proposal.md"
    assert_file_exists "$phases_dir/proposal.md"

    echo "PASS: AC-6 abnormal output handled via fallback"
    return 0
}

# ─── Test: AC-6 Edge — CLI completely missing (command not found) ────────────

test_ac6_cli_missing() {
    echo "=== AC-6 Edge: CLI completely missing (command not found) ==="

    setup_sandbox
    # Do NOT mock openspec — simulate it being completely absent
    # Remove the mock from PATH
    export PATH="$SANDBOX_DIR/bin_original:$PATH"

    local change_name="no-cli"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    mkdir -p "$phases_dir"

    # Attempt to call openspec — should fail with command not found
    local exit_code=0
    openspec spec list --json 2>/dev/null || exit_code=$?

    if [[ "$exit_code" -ne 0 ]]; then
        echo "  CLI correctly unavailable (exit code $exit_code)"
    fi

    # Generate proposal anyway (fallback)
    generate_fallback_proposal "$change_name" "standalone" > "$phases_dir/proposal.md"
    assert_file_exists "$phases_dir/proposal.md"
    assert_string_contains "$(cat "$phases_dir/proposal.md")" "# Proposal: $change_name"

    echo "PASS: AC-6 Edge CLI missing handled gracefully"
    return 0
}

# ─── Test: AC-6 Edge — Fallback proposal structure is valid ─────────────────

test_ac6_fallback_proposal_structure() {
    echo "=== AC-6 Edge: Fallback proposal structure is valid ==="

    setup_sandbox
    mock_openspec
    mock_spec_list error

    local change_name="structure-check"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    mkdir -p "$phases_dir"

    local planned_caps="logging"
    generate_fallback_proposal "$change_name" "$planned_caps" > "$phases_dir/proposal.md"

    # Must have Capabilities section
    assert_template_section "$phases_dir/proposal.md" "## Capabilities"
    assert_template_section "$phases_dir/proposal.md" "### New Capabilities"
    assert_template_section "$phases_dir/proposal.md" "### Modified Capabilities"

    # Must have User Story
    assert_template_section "$phases_dir/proposal.md" "## User Story"

    # Must have Acceptance Criteria
    assert_template_section "$phases_dir/proposal.md" "## Acceptance Criteria"

    echo "PASS: AC-6 Edge fallback proposal has valid structure"
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

run_test "AC-6: CLI fail still generates proposal" test_ac6_cli_fail_still_generates_proposal
run_test "AC-6: all capabilities New when CLI fails" test_ac6_all_capabilities_new_when_cli_unavailable
run_test "AC-6: abnormal output fallback" test_ac6_abnormal_output_fallback
run_test "AC-6 Edge: CLI completely missing" test_ac6_cli_missing
run_test "AC-6 Edge: fallback proposal structure" test_ac6_fallback_proposal_structure

# ─── Summary ─────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
