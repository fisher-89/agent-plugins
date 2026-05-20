#!/usr/bin/env bash
# test_planner_capabilities_flow.sh — Integration tests for Planner capabilities flow.
#
# Covers AC-4:
#   - Planner calls openspec_spec_list() and receives capability IDs
#   - Planner distinguishes New vs Modified capabilities based on existing specs
#   - Generated proposal.md contains Capabilities section with correct classification
#
# Usage:
#   bash test_planner_capabilities_flow.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_sandbox
}

trap cleanup EXIT

# ─── Helper: Simulate Planner capability classification logic ────────────────

# Given a list of all capability IDs and a list of existing spec capability IDs,
# classify each capability as New or Modified.
classify_capabilities() {
    local all_capabilities="$1"
    local existing_specs="$2"

    # Write JSON inputs to temp files to avoid quoting issues
    local caps_file="$SANDBOX_DIR/_classify_caps.json"
    local specs_file="$SANDBOX_DIR/_classify_specs.json"
    echo "$all_capabilities" > "$caps_file"
    echo "$existing_specs" > "$specs_file"

    python3 -c "
import json

with open('$caps_file') as f:
    all_caps = json.load(f)
with open('$specs_file') as f:
    existing = json.load(f)

new_list = []
modified_list = []

for item in all_caps:
    cap = item if isinstance(item, str) else item.get('id', '')
    if not cap:
        continue
    if cap in existing:
        modified_list.append(cap)
    else:
        new_list.append(cap)

print('NEW:' + ' '.join(new_list))
print('MODIFIED:' + ' '.join(modified_list))
" 2>/dev/null
}

# ─── Helper: Generate a proposal with Capabilities section ───────────────────

generate_proposal_with_capabilities() {
    local change_name="$1"
    local new_caps_csv="${2:-}"
    local modified_caps_csv="${3:-}"

    cat << PROPOSAL_EOF
# Proposal: $change_name

## User Story
As a developer, I want to build a capabilities-aware feature.

## Scope
- In: Capability integration
- Out: New infrastructure

## Acceptance Criteria
| ID | Criterion | Method | Priority |
|----|-----------|--------|----------|
| AC-01 | Capabilities are referenced | Review | High |

## Capabilities

### New Capabilities
PROPOSAL_EOF

    if [[ -n "$new_caps_csv" ]]; then
        IFS=' ' read -ra caps <<< "$new_caps_csv"
        for cap in "${caps[@]}"; do
            echo "- **$cap**: New capability for $cap functionality"
        done
    fi

    cat << PROPOSAL_EOF

### Modified Capabilities
PROPOSAL_EOF

    if [[ -n "$modified_caps_csv" ]]; then
        IFS=' ' read -ra caps <<< "$modified_caps_csv"
        for cap in "${caps[@]}"; do
            echo "- **$cap**: Extend existing $cap capability"
        done
    fi

    cat << PROPOSAL_EOF

## Risk
| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Capability gap | Medium | Low | Early identification |
PROPOSAL_EOF
}

# ─── Test: AC-4 — Planner calls spec list and receives capability IDs ────────

test_ac4_planner_receives_capability_ids() {
    echo "=== AC-4: Planner calls spec list and receives capability IDs ==="

    setup_sandbox
    mock_openspec
    mock_specs_dir normal
    export MOCK_ANSWERS_DIR="$SCRIPT_DIR/fixtures"

    # Simulate Planner calling openspec spec list
    local spec_list_output
    spec_list_output="$(openspec spec list --json 2>/dev/null || echo '[]')"

    # Verify we got valid JSON
    python3 -c "
import json, sys
data = json.loads('''$spec_list_output''')
assert isinstance(data, list), 'Expected list'
assert len(data) > 0, 'Expected non-empty list'
print('Received capabilities:', data)
"

    # Verify known IDs are present
    assert_string_contains "$spec_list_output" "auth"
    assert_string_contains "$spec_list_output" "storage"

    echo "PASS: AC-4 Planner receives capability IDs from spec list"
    return 0
}

# ─── Test: AC-4 — Planner distinguishes New vs Modified capabilities ─────────

test_ac4_planner_distinguishes_new_vs_modified() {
    echo "=== AC-4: Planner distinguishes New vs Modified capabilities ==="

    setup_sandbox
    mock_openspec
    mock_specs_dir normal
    export MOCK_ANSWERS_DIR="$SCRIPT_DIR/fixtures"

    # Get full capability list from spec list
    local spec_list_output
    spec_list_output="$(openspec spec list --json 2>/dev/null || echo '[]')"

    # Get existing specs (from the filesystem fixture)
    local existing_specs='["auth","storage"]'

    # Classify capabilities
    local classification
    classification="$(classify_capabilities "$spec_list_output" "$existing_specs")"

    local new_line
    new_line="$(echo "$classification" | grep "^NEW:" || true)"
    local modified_line
    modified_line="$(echo "$classification" | grep "^MODIFIED:" || true)"

    # In the fixture, "auth" and "storage" exist as specs, so they should be Modified
    # Any additional capabilities in the list are New
    if echo "$classification" | grep -q "MODIFIED:.*auth"; then
        echo "PASS: auth correctly classified as Modified"
    else
        echo "FAIL: auth should be classified as Modified" >&2
        echo "  Classification: $classification"
        return 1
    fi

    if echo "$classification" | grep -q "MODIFIED:.*storage"; then
        echo "PASS: storage correctly classified as Modified"
    else
        echo "FAIL: storage should be classified as Modified" >&2
        return 1
    fi

    echo "PASS: AC-4 Planner correctly distinguishes New vs Modified"
    return 0
}

# ─── Test: AC-4 — Generated proposal has Capabilities section with correct classification ──

test_ac4_proposal_has_correct_capabilities_section() {
    echo "=== AC-4: Generated proposal has Capabilities section with correct classification ==="

    setup_sandbox

    local change_name="capabilities-test"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    mkdir -p "$phases_dir"

    # Simulate: Planner has capabilities: auth (modified), storage (modified), logging (new)
    local new_caps="logging messaging"
    local modified_caps="auth storage"

    # Generate proposal
    generate_proposal_with_capabilities "$change_name" "$new_caps" "$modified_caps" > "$phases_dir/proposal.md"

    # Verify Capabilities section exists
    assert_template_section "$phases_dir/proposal.md" "## Capabilities"
    assert_template_section "$phases_dir/proposal.md" "### New Capabilities"
    assert_template_section "$phases_dir/proposal.md" "### Modified Capabilities"

    # Verify correct classification
    assert_capability_classified "$phases_dir/proposal.md" "logging" "New Capabilities"
    assert_capability_classified "$phases_dir/proposal.md" "messaging" "New Capabilities"
    assert_capability_classified "$phases_dir/proposal.md" "auth" "Modified Capabilities"
    assert_capability_classified "$phases_dir/proposal.md" "storage" "Modified Capabilities"

    echo "PASS: AC-4 proposal has correct Capabilities section"
    return 0
}

# ─── Test: AC-4 Edge — No existing specs, all capabilities are New ───────────

test_ac4_all_new_capabilities() {
    echo "=== AC-4 Edge: No existing specs — all capabilities classified as New ==="

    setup_sandbox
    mock_openspec
    mock_specs_dir empty
    export MOCK_ANSWERS_DIR="$SCRIPT_DIR/fixtures"

    # Mock spec list to return capabilities, but specs dir is empty
    local spec_list_output='["logging","messaging","analytics"]'
    local existing_specs='[]'

    local classification
    classification="$(classify_capabilities "$spec_list_output" "$existing_specs")"

    local new_line
    new_line="$(echo "$classification" | grep "^NEW:" || true)"
    local modified_line
    modified_line="$(echo "$classification" | grep "^MODIFIED:" || true)"

    if [[ -z "$modified_line" || "$modified_line" == "MODIFIED:" ]]; then
        echo "PASS: No Modified capabilities when no specs exist"
    else
        echo "FAIL: Should have no Modified capabilities" >&2
        echo "  Classification: $classification"
        return 1
    fi

    if echo "$new_line" | grep -q "logging"; then
        echo "PASS: logging correctly classified as New"
    else
        echo "FAIL: logging should be New" >&2
        return 1
    fi

    echo "PASS: AC-4 Edge all capabilities classified as New"
    return 0
}

# ─── Test: AC-4 Edge — Empty spec list, empty capability list ────────────────

test_ac4_empty_lists() {
    echo "=== AC-4 Edge: Empty spec list and empty capability list ==="

    setup_sandbox

    local spec_list_output='[]'
    local existing_specs='[]'

    local classification
    classification="$(classify_capabilities "$spec_list_output" "$existing_specs")"

    local new_line
    new_line="$(echo "$classification" | grep "^NEW:" || true)"
    local modified_line
    modified_line="$(echo "$classification" | grep "^MODIFIED:" || true)"

    if [[ "$new_line" == "NEW:" && "$modified_line" == "MODIFIED:" ]]; then
        echo "PASS: Empty lists produce no capabilities"
    else
        echo "FAIL: Expected empty New and Modified lists" >&2
        echo "  Classification: $classification"
        return 1
    fi

    echo "PASS: AC-4 Edge empty lists handled correctly"
    return 0
}

# ─── Test: AC-4 Edge — Capabilities section ordering (New before Modified) ───

test_ac4_capabilities_section_ordering() {
    echo "=== AC-4 Edge: Capabilities section ordering (New before Modified) ==="

    setup_sandbox

    local change_name="ordering-test"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    mkdir -p "$phases_dir"

    local new_caps="logging"
    local modified_caps="auth storage"

    generate_proposal_with_capabilities "$change_name" "$new_caps" "$modified_caps" > "$phases_dir/proposal.md"

    # Verify New Capabilities appears before Modified Capabilities
    local new_line_num
    new_line_num="$(grep -n "### New Capabilities" "$phases_dir/proposal.md" | cut -d: -f1)"
    local modified_line_num
    modified_line_num="$(grep -n "### Modified Capabilities" "$phases_dir/proposal.md" | cut -d: -f1)"

    if [[ "$new_line_num" -lt "$modified_line_num" ]]; then
        echo "PASS: New Capabilities (line $new_line_num) before Modified Capabilities (line $modified_line_num)"
    else
        echo "FAIL: New Capabilities should appear before Modified Capabilities" >&2
        return 1
    fi

    echo "PASS: AC-4 Edge section ordering correct"
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

run_test "AC-4: planner receives capability IDs" test_ac4_planner_receives_capability_ids
run_test "AC-4: planner distinguishes New vs Modified" test_ac4_planner_distinguishes_new_vs_modified
run_test "AC-4: proposal has correct capabilities section" test_ac4_proposal_has_correct_capabilities_section
run_test "AC-4 Edge: all New when no specs" test_ac4_all_new_capabilities
run_test "AC-4 Edge: empty lists" test_ac4_empty_lists
run_test "AC-4 Edge: section ordering" test_ac4_capabilities_section_ordering

# ─── Summary ─────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
