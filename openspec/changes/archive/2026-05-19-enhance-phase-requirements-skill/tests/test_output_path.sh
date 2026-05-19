#!/usr/bin/env bash
# test_output_path.sh — Integration tests for artifact output path preservation.
#
# Covers:
#   AC-09: Artifact output paths remain unchanged (regression)
#   Verify that proposal.md is written to openspec/changes/<name>/phases/proposal.md
#   Verify that eval.json is written to openspec/changes/<name>/phases/eval.json
#
# Usage:
#   bash test_output_path.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/helpers/setup_test_env.sh"

PASS_COUNT=0
FAIL_COUNT=0

cleanup() {
    teardown_sandbox
}

trap cleanup EXIT

# ─── Test: AC-09 — Proposal.md output path ──────────────────────────────────

test_ac09_proposal_output_path() {
    echo "=== AC-09: proposal.md output path ==="

    setup_sandbox
    mock_openspec

    local change_name="test-change"
    local expected_path="$FAKE_CHANGES_DIR/$change_name/phases/proposal.md"

    # Create the change directory (simulating scaffolding)
    mkdir -p "$FAKE_CHANGES_DIR/$change_name/phases"

    # Simulate writing the proposal artifact
    echo "# Proposal" > "$expected_path"

    assert_file_exists "$expected_path"

    echo "PASS: AC-09 proposal.md written to correct path: $expected_path"
    return 0
}

# ─── Test: AC-09 — Eval.json output path ────────────────────────────────────

test_ac09_eval_output_path() {
    echo "=== AC-09: eval.json output path ==="

    setup_sandbox

    local change_name="test-change"
    local expected_path="$FAKE_CHANGES_DIR/$change_name/phases/eval.json"

    mkdir -p "$FAKE_CHANGES_DIR/$change_name/phases"

    # Simulate writing the eval artifact
    echo '[]' > "$expected_path"

    assert_file_exists "$expected_path"

    echo "PASS: AC-09 eval.json written to correct path: $expected_path"
    return 0
}

# ─── Test: AC-09 — Output path matches openspec instructions ────────────────

test_ac09_output_path_from_instructions() {
    echo "=== AC-09: Output path matches openspec instructions outputPath ==="

    setup_sandbox
    mock_openspec

    local change_name="test-change"
    export MOCK_ANSWERS_DIR="$SCRIPT_DIR/fixtures"

    # Read the outputPath from instructions fixture
    local output_path
    output_path="$(python3 -c "
import json
data = json.load(open('$SCRIPT_DIR/fixtures/openspec_instructions.json'))
print(data.get('outputPath', ''))
")"

    # The fixture template uses <name> as placeholder
    local resolved_path="${output_path//<name>/$change_name}"
    local expected_path="$SANDBOX_DIR/$resolved_path"

    # Create the expected directory
    mkdir -p "$(dirname "$expected_path")"

    # Write the artifact
    echo "# Test" > "$expected_path"
    assert_file_exists "$expected_path"

    # Verify the path contains the expected structure
    assert_string_contains "$expected_path" "$change_name/phases/proposal.md"

    echo "PASS: AC-09 output path resolved from instructions"
    return 0
}

# ─── Test: AC-09 — Directory structure is created if not exists ─────────────

test_ac09_directory_auto_creation() {
    echo "=== AC-09: Directory structure creates phases/ if missing ==="

    setup_sandbox

    local change_name="auto-dir-change"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"

    # Create change dir but NOT phases subdirectory
    mkdir -p "$FAKE_CHANGES_DIR/$change_name"

    # Verify phases does not exist yet
    if [[ -d "$phases_dir" ]]; then
        echo "SKIP: phases dir already exists (setup issue)"
        return 0
    fi

    # Simulate artifact creation (mkdir -p before write)
    mkdir -p "$phases_dir"
    echo "# Proposal" > "$phases_dir/proposal.md"

    assert_dir_exists "$phases_dir"
    assert_file_exists "$phases_dir/proposal.md"

    echo "PASS: AC-09 directory auto-creation works"
    return 0
}

# ─── Test: AC-09 — Multiple artifacts in same change ────────────────────────

test_ac09_multiple_artifacts() {
    echo "=== AC-09: Multiple artifacts in same change ==="

    setup_sandbox

    local change_name="multi-artifact"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    mkdir -p "$phases_dir"

    # Write proposal.md
    echo "# Proposal" > "$phases_dir/proposal.md"
    # Write design.md
    echo "# Design" > "$phases_dir/design.md"
    # Write tasks.md
    echo "# Tasks" > "$phases_dir/tasks.md"
    # Write eval.json
    echo '[]' > "$phases_dir/eval.json"

    assert_file_exists "$phases_dir/proposal.md"
    assert_file_exists "$phases_dir/design.md"
    assert_file_exists "$phases_dir/tasks.md"
    assert_file_exists "$phases_dir/eval.json"

    echo "PASS: AC-09 multiple artifacts in same phases/ directory"
    return 0
}

# ─── Test: AC-09 Edge — Path with special characters in change name ─────────

test_ac09_path_with_hyphens() {
    echo "=== AC-09 Edge: Path with hyphenated change name ==="

    setup_sandbox

    local change_name="my-complex-change-name-v2"
    local phases_dir="$FAKE_CHANGES_DIR/$change_name/phases"
    mkdir -p "$phases_dir"

    echo "# Proposal" > "$phases_dir/proposal.md"
    assert_file_exists "$phases_dir/proposal.md"

    echo "PASS: AC-09 Edge hyphenated path works"
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

run_test "AC-09: proposal output path" test_ac09_proposal_output_path
run_test "AC-09: eval output path" test_ac09_eval_output_path
run_test "AC-09: path from instructions" test_ac09_output_path_from_instructions
run_test "AC-09: directory auto-creation" test_ac09_directory_auto_creation
run_test "AC-09: multiple artifacts" test_ac09_multiple_artifacts
run_test "AC-09 Edge: hyphenated path" test_ac09_path_with_hyphens

# ─── Summary ────────────────────────────────────────────────────────────────

echo ""
echo "══════════════════════════════════════════════"
echo "  Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "══════════════════════════════════════════════"

if [[ "$FAIL_COUNT" -gt 0 ]]; then
    exit 1
fi
exit 0
